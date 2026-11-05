// Minimal SQLite C declarations (Zig 0.17 has no @cImport).
pub const sqlite3 = opaque {};
pub const sqlite3_stmt = opaque {};

pub const SQLITE_OK: c_int = 0;
pub const SQLITE_ROW: c_int = 100;
pub const SQLITE_DONE: c_int = 101;
pub const SQLITE_BUSY: c_int = 5;
pub const SQLITE_NULL: c_int = 5;
pub const SQLITE_INTEGER: c_int = 1;
pub const SQLITE_FLOAT: c_int = 2;
pub const SQLITE_BLOB: c_int = 4;

pub extern fn sqlite3_open(filename: [*:0]const u8, ppDb: *?*sqlite3) c_int;
pub extern fn sqlite3_close(db: ?*sqlite3) c_int;
pub extern fn sqlite3_errmsg(db: ?*sqlite3) [*c]const u8;
pub extern fn sqlite3_last_insert_rowid(db: ?*sqlite3) i64;
pub extern fn sqlite3_changes(db: ?*sqlite3) c_int;
pub extern fn sqlite3_free(ptr: ?*anyopaque) void;
pub extern fn sqlite3_exec(
    db: ?*sqlite3,
    sql: [*:0]const u8,
    callback: ?*const fn (?*anyopaque, c_int, [*c][*c]u8, [*c][*c]u8) callconv(.c) c_int,
    arg: ?*anyopaque,
    errmsg: ?*[*c]u8,
) c_int;
pub extern fn sqlite3_prepare_v2(
    db: ?*sqlite3,
    sql: [*:0]const u8,
    n_byte: c_int,
    stmt: *?*sqlite3_stmt,
    tail: ?*[*c]u8,
) c_int;
pub extern fn sqlite3_finalize(stmt: ?*sqlite3_stmt) c_int;
pub extern fn sqlite3_reset(stmt: ?*sqlite3_stmt) c_int;
pub extern fn sqlite3_clear_bindings(stmt: ?*sqlite3_stmt) c_int;
pub extern fn sqlite3_step(stmt: ?*sqlite3_stmt) c_int;
pub extern fn sqlite3_bind_null(stmt: ?*sqlite3_stmt, index: c_int) c_int;
pub extern fn sqlite3_bind_int(stmt: ?*sqlite3_stmt, index: c_int, value: c_int) c_int;
pub extern fn sqlite3_bind_int64(stmt: ?*sqlite3_stmt, index: c_int, value: i64) c_int;
pub extern fn sqlite3_bind_double(stmt: ?*sqlite3_stmt, index: c_int, value: f64) c_int;
pub extern fn sqlite3_bind_text(
    stmt: ?*sqlite3_stmt,
    index: c_int,
    value: [*]const u8,
    length: c_int,
    destructor: ?*const fn (?*anyopaque) callconv(.c) void,
) c_int;
pub extern fn sqlite3_column_count(stmt: ?*sqlite3_stmt) c_int;
pub extern fn sqlite3_column_type(stmt: ?*sqlite3_stmt, index: c_int) c_int;
pub extern fn sqlite3_column_int64(stmt: ?*sqlite3_stmt, index: c_int) i64;
pub extern fn sqlite3_column_double(stmt: ?*sqlite3_stmt, index: c_int) f64;
pub extern fn sqlite3_column_text(stmt: ?*sqlite3_stmt, index: c_int) [*c]const u8;
pub extern fn sqlite3_column_blob(stmt: ?*sqlite3_stmt, index: c_int) ?*const anyopaque;
pub extern fn sqlite3_column_bytes(stmt: ?*sqlite3_stmt, index: c_int) c_int;
