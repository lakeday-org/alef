use super::ZigStreamingAdapterMetadata;
use std::fmt::Write;

/// Emit the error-path body of a streaming fixture through the generated Zig wrapper.
///
/// The call goes through the wrapper's stream-start method and the stream's `next()`, so
/// the error the test sees is the one the wrapper returns, not the one the C library
/// still holds in its thread-local state. ~keep
pub(super) fn render(out: &mut String, adapter: &ZigStreamingAdapterMetadata, args: &str) {
    let name = &adapter.adapter_name;
    let _ = writeln!(
        out,
        r#"    const _stream_result: anyerror!void = stream_result: {{
        var _stream = _client.{name}({args}) catch |_start_err| break :stream_result _start_err;
        defer _stream.deinit();
        while (true) {{
            const _chunk = _stream.next() catch |_next_err| break :stream_result _next_err;
            if (_chunk == null) break :stream_result {{}};
        }}
    }};
    if (_stream_result) |_| {{"#
    );
}
