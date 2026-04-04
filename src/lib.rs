//! SQLite FTS5 Korean tokenizer extension powered by garu-core.
//!
//! # Build
//!
//! ```sh
//! cargo build --release --features loadable
//! ```
//!
//! # Load
//!
//! ```sql
//! .load libsqlite_garu
//! ```
//!
//! # Use
//!
//! ```sql
//! CREATE VIRTUAL TABLE t USING fts5(content, tokenize='garu');
//! INSERT INTO t VALUES ('달리는 사람');
//! INSERT INTO t VALUES ('나는 달렸다');
//! SELECT * FROM t WHERE t MATCH '달렸다';
//! -- → both rows returned (morphological search)
//! ```
//!
//! # Features
//!
//! - `loadable` (required for `.load`): Builds with `rusqlite/loadable_extension`
//!   so the dylib exports the SQLite extension entry points.
//!   **Omit this feature when running `cargo test`.**

use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use std::sync::OnceLock;

use garu_core::model::Analyzer;
use garu_core::types::Pos;
use rusqlite::ffi::{sqlite3, sqlite3_stmt, SQLITE_OK};

pub fn is_stop_pos(pos: Pos) -> bool {
    matches!(
        pos,
        Pos::EP
            | Pos::EF
            | Pos::EC
            | Pos::ETN
            | Pos::ETM
            | Pos::IC
            | Pos::JKS
            | Pos::JKC
            | Pos::JKG
            | Pos::JKO
            | Pos::JKB
            | Pos::JKV
            | Pos::JKQ
            | Pos::JX
            | Pos::JC
            | Pos::MAG
            | Pos::MAJ
            | Pos::MM
            | Pos::SF
            | Pos::SP
            | Pos::SS
            | Pos::SE
            | Pos::SO
            | Pos::SW
            | Pos::XPN
            | Pos::XSA
            | Pos::XSN
            | Pos::XSV
    )
}

unsafe extern "C" {
    fn sqlite3_prepare_v2(
        db: *mut sqlite3,
        z_sql: *const c_char,
        n_byte: c_int,
        pp_stmt: *mut *mut sqlite3_stmt,
        pz_tail: *mut *const c_char,
    ) -> c_int;
    fn sqlite3_bind_pointer(
        stmt: *mut sqlite3_stmt,
        idx: c_int,
        ptr: *mut c_void,
        type_tag: *const c_char,
        destructor: Option<unsafe extern "C" fn(*mut c_void)>,
    ) -> c_int;
    fn sqlite3_step(stmt: *mut sqlite3_stmt) -> c_int;
    fn sqlite3_finalize(stmt: *mut sqlite3_stmt) -> c_int;
}

// ---------------------------------------------------------------------------
// Embedded model
// ---------------------------------------------------------------------------

const BASE_GMDL: &[u8] = include_bytes!("../_ref/garu/js/models/base.gmdl");
const CNN2_BIN: &[u8] = include_bytes!("../_ref/garu/js/models/cnn2.bin");

static ANALYZER: OnceLock<Analyzer> = OnceLock::new();

fn get_analyzer() -> &'static Analyzer {
    ANALYZER.get_or_init(|| {
        Analyzer::from_bytes(BASE_GMDL, CNN2_BIN).expect("Failed to load embedded garu model")
    })
}

// ---------------------------------------------------------------------------
// FTS5 FFI types (matching sqlite3/ext/fts5.h definitions)
// ---------------------------------------------------------------------------

/// Opaque type for FTS5 tokenizer instances.
#[repr(C)]
pub struct Fts5Tokenizer {
    _unused: [u8; 0],
}

/// FTS5 tokenizer callback struct (matches fts5_tokenizer from fts5.h).
#[repr(C)]
pub struct Fts5TokenizerCallbacks {
    pub x_create: Option<
        unsafe extern "C" fn(
            p_ctx: *mut c_void,
            az_arg: *mut *const c_char,
            n_arg: c_int,
            pp_out: *mut *mut Fts5Tokenizer,
        ) -> c_int,
    >,
    pub x_delete: Option<unsafe extern "C" fn(p_tok: *mut Fts5Tokenizer)>,
    pub x_tokenize: Option<
        unsafe extern "C" fn(
            p_tok: *mut Fts5Tokenizer,
            p_ctx: *mut c_void,
            flags: c_int,
            p_text: *const c_char,
            n_text: c_int,
            x_token: Option<
                unsafe extern "C" fn(
                    p_ctx: *mut c_void,
                    tflags: c_int,
                    p_token: *const c_char,
                    n_token: c_int,
                    i_start: c_int,
                    i_end: c_int,
                    i_pos: c_int,
                ) -> c_int,
            >,
        ) -> c_int,
    >,
}

/// FTS5 extension API struct (matches fts5_api from fts5.h).
#[repr(C)]
pub struct Fts5Api {
    pub i_version: c_int,
    pub x_create_tokenizer: Option<
        unsafe extern "C" fn(
            p_api: *mut Fts5Api,
            z_name: *const c_char,
            p_ctx: *mut c_void,
            p_tok: *mut Fts5TokenizerCallbacks,
            x_destroy: Option<unsafe extern "C" fn(*mut c_void)>,
        ) -> c_int,
    >,
    pub x_find_tokenizer: Option<
        unsafe extern "C" fn(
            p_api: *mut Fts5Api,
            z_name: *const c_char,
            pp_ctx: *mut *mut c_void,
            p_tok: *mut Fts5TokenizerCallbacks,
        ) -> c_int,
    >,
    pub x_create_function: Option<
        unsafe extern "C" fn(
            p_api: *mut Fts5Api,
            z_name: *const c_char,
            x_func: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, c_int, *mut c_void)>,
            p_ctx: *mut c_void,
            x_destroy: Option<unsafe extern "C" fn(*mut c_void)>,
        ) -> c_int,
    >,
}

// ---------------------------------------------------------------------------
// FTS5 API retrieval via SELECT fts5(?1)
// ---------------------------------------------------------------------------

/// Retrieve the FTS5 API pointer from the database connection.
/// Uses the official method: prepare "SELECT fts5(?1)", bind a pointer,
/// step, and read back the fts5_api* from the bound output.
unsafe fn get_fts5_api(db: *mut sqlite3) -> Option<*mut Fts5Api> {
    let mut api_ptr: *mut Fts5Api = ptr::null_mut();
    let mut stmt: *mut sqlite3_stmt = ptr::null_mut();

    let sql = CString::new("SELECT fts5(?1)").ok()?;
    let label = CString::new("fts5_api_ptr").ok()?;

    let rc = sqlite3_prepare_v2(db, sql.as_ptr(), -1, &mut stmt, ptr::null_mut());
    if rc != SQLITE_OK as c_int {
        return None;
    }

    let rc = sqlite3_bind_pointer(
        stmt,
        1,
        &mut api_ptr as *mut _ as *mut c_void,
        label.as_ptr(),
        None,
    );
    if rc != SQLITE_OK as c_int {
        sqlite3_finalize(stmt);
        return None;
    }

    sqlite3_step(stmt);
    sqlite3_finalize(stmt);

    if api_ptr.is_null() {
        None
    } else {
        Some(api_ptr)
    }
}

// ---------------------------------------------------------------------------
// Tokenizer callbacks
// ---------------------------------------------------------------------------

struct GaruTokenizer;

unsafe extern "C" fn garu_create(
    _p_ctx: *mut c_void,
    _az_arg: *mut *const c_char,
    _n_arg: c_int,
    pp_out: *mut *mut Fts5Tokenizer,
) -> c_int {
    // Pre-initialize the analyzer
    let _ = get_analyzer();
    let tokenizer = Box::new(GaruTokenizer);
    *pp_out = Box::into_raw(tokenizer) as *mut Fts5Tokenizer;
    SQLITE_OK as c_int
}

unsafe extern "C" fn garu_delete(p_tok: *mut Fts5Tokenizer) {
    if !p_tok.is_null() {
        let _ = Box::from_raw(p_tok as *mut GaruTokenizer);
    }
}

unsafe extern "C" fn garu_tokenize(
    _p_tok: *mut Fts5Tokenizer,
    p_ctx: *mut c_void,
    _flags: c_int,
    p_text: *const c_char,
    n_text: c_int,
    x_token: Option<
        unsafe extern "C" fn(
            p_ctx: *mut c_void,
            tflags: c_int,
            p_token: *const c_char,
            n_token: c_int,
            i_start: c_int,
            i_end: c_int,
            i_pos: c_int,
        ) -> c_int,
    >,
) -> c_int {
    let x_token = match x_token {
        Some(f) => f,
        None => return SQLITE_OK as c_int,
    };

    if n_text <= 0 || p_text.is_null() {
        return SQLITE_OK as c_int;
    }

    let input_bytes = std::slice::from_raw_parts(p_text as *const u8, n_text as usize);
    let input = match std::str::from_utf8(input_bytes) {
        Ok(s) => s,
        Err(_) => return 1,
    };

    let analyzer = get_analyzer();
    let result = analyzer.analyze(input);

    let mut token_pos = 0i32;
    for token in &result.tokens {
        if is_stop_pos(token.pos) {
            continue;
        }

        let token_bytes = token.text.as_bytes();
        let rc = x_token(
            p_ctx,
            0, // tflags: 0 = plain token (not coalesced)
            token_bytes.as_ptr() as *const c_char,
            token_bytes.len() as c_int,
            token.start as c_int,
            token.end as c_int,
            token_pos,
        );

        if rc != SQLITE_OK as c_int {
            return rc;
        }
        token_pos += 1;
    }

    SQLITE_OK as c_int
}

// ---------------------------------------------------------------------------
// Static tokenizer callback vtable
// ---------------------------------------------------------------------------

static TOKENIZER_CALLBACKS: Fts5TokenizerCallbacks = Fts5TokenizerCallbacks {
    x_create: Some(garu_create),
    x_delete: Some(garu_delete),
    x_tokenize: Some(garu_tokenize),
};

// ---------------------------------------------------------------------------
// Extension entry points
// ---------------------------------------------------------------------------

pub unsafe fn register_tokenizer(db: *mut sqlite3) -> c_int {
    let fts5_api = match get_fts5_api(db) {
        Some(api) => api,
        None => return 1,
    };

    let name = match CString::new("garu") {
        Ok(n) => n,
        Err(_) => return 1,
    };

    let x_create_tokenizer = match (*fts5_api).x_create_tokenizer {
        Some(f) => f,
        None => return 1,
    };

    x_create_tokenizer(
        fts5_api,
        name.as_ptr(),
        ptr::null_mut(),
        &TOKENIZER_CALLBACKS as *const Fts5TokenizerCallbacks as *mut Fts5TokenizerCallbacks,
        None,
    )
}

#[cfg(feature = "loadable")]
mod entry_points {
    use super::*;
    use rusqlite::ffi::{self, sqlite3_api_routines};

    #[no_mangle]
    pub unsafe extern "C" fn sqlite3_sqlite_garu_init(
        db: *mut sqlite3,
        _pz_err_msg: *mut *mut c_char,
        p_api: *mut sqlite3_api_routines,
    ) -> c_int {
        if ffi::rusqlite_extension_init2(p_api).is_err() {
            return ffi::SQLITE_ERROR;
        }
        register_tokenizer(db)
    }

    #[no_mangle]
    pub unsafe extern "C" fn sqlite3_sqlitegaru_init(
        db: *mut sqlite3,
        _pz_err_msg: *mut *mut c_char,
        p_api: *mut sqlite3_api_routines,
    ) -> c_int {
        if ffi::rusqlite_extension_init2(p_api).is_err() {
            return ffi::SQLITE_ERROR;
        }
        register_tokenizer(db)
    }

    #[no_mangle]
    pub unsafe extern "C" fn sqlite3_garu_init(
        db: *mut sqlite3,
        _pz_err_msg: *mut *mut c_char,
        p_api: *mut sqlite3_api_routines,
    ) -> c_int {
        if ffi::rusqlite_extension_init2(p_api).is_err() {
            return ffi::SQLITE_ERROR;
        }
        register_tokenizer(db)
    }
}
