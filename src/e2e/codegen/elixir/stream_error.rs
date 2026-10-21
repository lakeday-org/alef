//! Error-path body for a streaming fixture in the generated Elixir tests.

use std::fmt::Write as _;

/// Bind the outcome of a streaming call and return the name of the binding.
///
/// The Elixir stream is lazy. The call returns `{:ok, stream}` and a mid-stream failure raises
/// while the stream is read, so a bare `assert {:error, _} = call` fails for a stream that
/// ends in an error. The binding reads the stream to its end and turns a raised exception
/// into `{:error, exception}`. A stream that ends without an error gives
/// `:stream_ended_without_error`, which the `{:error, _}` match rejects.
pub(super) fn bind_stream_outcome(out: &mut String, indent: &str, call_expr: &str) -> &'static str {
    let _ = writeln!(out, "{indent}__stream_outcome =");
    let _ = writeln!(out, "{indent}  try do");
    let _ = writeln!(out, "{indent}    case {call_expr} do");
    let _ = writeln!(out, "{indent}      {{:ok, __stream}} ->");
    let _ = writeln!(out, "{indent}        Enum.each(__stream, fn _chunk -> :ok end)");
    let _ = writeln!(out, "{indent}        :stream_ended_without_error");
    let _ = writeln!(out);
    let _ = writeln!(out, "{indent}      {{:error, _}} = __error ->");
    let _ = writeln!(out, "{indent}        __error");
    let _ = writeln!(out, "{indent}    end");
    let _ = writeln!(out, "{indent}  rescue");
    let _ = writeln!(out, "{indent}    __exception -> {{:error, __exception}}");
    let _ = writeln!(out, "{indent}  end");
    let _ = writeln!(out);
    "__stream_outcome"
}
