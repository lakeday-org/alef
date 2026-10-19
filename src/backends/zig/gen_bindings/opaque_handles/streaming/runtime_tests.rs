//! Compile and run the Zig that the streaming emitters write.
//!
//! The text tests next to the templates prove that a line is present. These tests prove what
//! the generated `start` and `next` do: a stub C layer plays a failing library, and Zig tests
//! check the error that each failure returns and the message the wrapper keeps.

use super::{StreamingContext, emit_opaque_streaming_method, emit_streaming_struct};
use crate::backends::zig::gen_bindings::errors::emit_error_set;
use crate::backends::zig::gen_bindings::helpers::emit_helpers;
use crate::core::ir::{ErrorDef, ErrorVariant, MethodDef, ParamDef, TypeDef, TypeRef};
use std::collections::HashMap;

/// A C layer in Zig. Every `*_free` call clears the error state, as `catch_ffi_panic` does in
/// the real FFI crate, so a wrapper that reads the error late sees nothing.
const STUB_C_LAYER: &str = r#"const std = @import("std");

const c = struct {
    pub var code: i32 = 0;
    pub var ctx: [*c]const u8 = null;
    pub var json: [*c]u8 = null;
    pub var next_result: u64 = 0;
    pub var start_result: u64 = 0;
    pub var request_result: u64 = 0;
    fn clear() void {
        code = 0;
        ctx = null;
    }
    pub fn sample_last_error_code() i32 {
        return code;
    }
    pub fn sample_last_error_context() [*c]const u8 {
        return ctx;
    }
    pub fn sample_free_string(p: [*c]u8) void {
        _ = p;
    }
    pub fn sample_record_to_json(chunk: u64) [*c]u8 {
        _ = chunk;
        return json;
    }
    pub fn sample_record_free(chunk: u64) void {
        _ = chunk;
        clear();
    }
    pub fn sample_client_stream_records_next(h: u64) u64 {
        _ = h;
        return next_result;
    }
    pub fn sample_client_stream_records_free(h: u64) void {
        _ = h;
        clear();
    }
    pub fn sample_record_request_from_json(p: [*c]const u8) u64 {
        _ = p;
        return request_result;
    }
    pub fn sample_record_request_free(h: u64) void {
        _ = h;
        clear();
    }
    pub fn sample_client_stream_records_start(h: u64, r: u64) u64 {
        _ = h;
        _ = r;
        return start_result;
    }
};

const Record = struct { id: u32 };

"#;

const ZIG_TESTS: &str = r#"
fn reset() void {
    c.code = 0;
    c.ctx = null;
    c.json = null;
    c.next_result = 0;
    c.start_result = 0;
    c.request_result = 0;
    if (_captured_error) |_old| std.heap.c_allocator.free(_old);
    _captured_error = null;
}

fn open_stream(client: *Client) !RecordStream {
    reset();
    c.request_result = 3;
    c.start_result = 7;
    return client.stream_records("{}");
}

test "a rejected stream start returns the native error and keeps its message" {
    reset();
    var client = Client{ ._handle = 1 };
    c.request_result = 3;
    c.start_result = 0;
    c.code = 100;
    c.ctx = "bad key";
    try std.testing.expectError(error.InvalidInput, client.stream_records("{}"));
    try std.testing.expectEqualStrings("bad key", _last_error().?);
}

test "a request that fails to marshal returns the native error and keeps its message" {
    reset();
    var client = Client{ ._handle = 1 };
    c.request_result = 0;
    c.code = 100;
    c.ctx = "bad request";
    try std.testing.expectError(error.InvalidInput, client.stream_records("{}"));
    try std.testing.expectEqualStrings("bad request", _last_error().?);
}

test "a start failure with an unknown native code returns UnknownFfiError" {
    reset();
    var client = Client{ ._handle = 1 };
    c.request_result = 3;
    c.code = 77;
    c.ctx = "unmapped";
    try std.testing.expectError(error.UnknownFfiError, client.stream_records("{}"));
}

test "a mid-stream failure returns the native error and keeps its message" {
    var client = Client{ ._handle = 1 };
    var stream = try open_stream(&client);
    defer stream.deinit();
    c.next_result = 0;
    c.code = 100;
    c.ctx = "mid-stream failure";
    try std.testing.expectError(error.InvalidInput, stream.next());
    try std.testing.expectEqualStrings("mid-stream failure", _last_error().?);
}

test "a cancelled stream returns Cancelled" {
    var client = Client{ ._handle = 1 };
    var stream = try open_stream(&client);
    defer stream.deinit();
    c.next_result = 0;
    c.code = 5;
    c.ctx = "cancelled by the caller";
    try std.testing.expectError(error.Cancelled, stream.next());
    try std.testing.expectEqualStrings("cancelled by the caller", _last_error().?);
}

test "the end of the stream returns null" {
    var client = Client{ ._handle = 1 };
    var stream = try open_stream(&client);
    defer stream.deinit();
    c.next_result = 0;
    c.code = 0;
    try std.testing.expectEqual(@as(?Record, null), try stream.next());
}

test "a chunk that fails to convert to JSON returns the native error" {
    var client = Client{ ._handle = 1 };
    var stream = try open_stream(&client);
    defer stream.deinit();
    c.next_result = 9;
    c.json = null;
    c.code = 100;
    c.ctx = "no json";
    try std.testing.expectError(error.InvalidInput, stream.next());
    try std.testing.expectEqualStrings("no json", _last_error().?);
}

test "a chunk that does not parse returns UnknownFfiError and never OutOfMemory" {
    var client = Client{ ._handle = 1 };
    var stream = try open_stream(&client);
    defer stream.deinit();
    c.next_result = 9;
    c.json = @ptrCast(@constCast("{bad"));
    try std.testing.expectError(error.UnknownFfiError, stream.next());
    c.json = @ptrCast(@constCast("{\"id\":\"not a number\"}"));
    try std.testing.expectError(error.UnknownFfiError, stream.next());
}

test "a chunk that parses is returned and unknown fields are ignored" {
    var client = Client{ ._handle = 1 };
    var stream = try open_stream(&client);
    defer stream.deinit();
    c.next_result = 9;
    c.json = @ptrCast(@constCast("{\"id\":42,\"extra\":true}"));
    const chunk = (try stream.next()).?;
    try std.testing.expectEqual(@as(u32, 42), chunk.id);
}
"#;

const ZIG_TEST_COUNT: usize = 9;

fn request_error() -> ErrorDef {
    ErrorDef {
        exception_base: None,
        name: "RequestError".to_string(),
        rust_path: "sample::RequestError".to_string(),
        variants: vec![ErrorVariant {
            error_code: Some(100),
            name: "InvalidInput".to_string(),
            is_unit: true,
            ..Default::default()
        }],
        original_rust_path: String::new(),
        doc: String::new(),
        methods: Vec::new(),
        binding_excluded: false,
        binding_exclusion_reason: None,
        version: Default::default(),
    }
}

/// The Zig source of a client with one streaming method, written by the real emitters.
fn generated_client_source() -> String {
    let error = request_error();
    let method = MethodDef {
        name: "stream_records".to_string(),
        params: vec![ParamDef {
            name: "req".to_string(),
            ty: TypeRef::Named("RecordRequest".to_string()),
            ..ParamDef::default()
        }],
        error_type: Some("RequestError".to_string()),
        ..MethodDef::default()
    };
    let ty = TypeDef {
        name: "Client".to_string(),
        methods: vec![method.clone()],
        ..TypeDef::default()
    };
    let declared_errors = vec!["RequestError".to_string()];
    let streaming_item_types = HashMap::from([("stream_records".to_string(), "Record".to_string())]);
    let streaming = StreamingContext {
        ty: &ty,
        prefix: "sample",
        type_snake: "client",
        item_type: "Record",
        declared_errors: &declared_errors,
        streaming_item_types: &streaming_item_types,
    };

    let mut out = String::from(STUB_C_LAYER);
    emit_helpers("sample", std::slice::from_ref(&error), &mut out);
    emit_error_set(&error, &mut out);
    emit_streaming_struct(&method, &streaming, &mut out);
    out.push_str("pub const Client = struct {\n    _handle: u64,\n\n");
    emit_opaque_streaming_method(&method, &streaming, &mut out);
    out.push_str("};\n");
    out.push_str(ZIG_TESTS);
    out
}

#[test]
fn generated_stream_wrapper_returns_the_native_error_at_runtime() {
    if std::process::Command::new("zig").arg("version").output().is_err() {
        return;
    }
    let directory = tempfile::tempdir().expect("temporary directory");
    let source_path = directory.path().join("stream.zig");
    std::fs::write(&source_path, generated_client_source()).expect("write Zig source");
    // `-lc` mirrors the `.link_libc = true` that every generated project sets: the wrapper
    // allocates through `std.heap.c_allocator`. ~keep
    let output = std::process::Command::new("zig")
        .args(["test", "-lc"])
        .arg(&source_path)
        .current_dir(directory.path())
        .output()
        .expect("run Zig compiler");
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "the generated stream wrapper failed:\n{report}"
    );
    assert!(
        report.contains(&format!("All {ZIG_TEST_COUNT} tests passed")),
        "the run must execute every Zig test:\n{report}"
    );
}
