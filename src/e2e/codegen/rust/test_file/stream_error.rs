//! Error-path body for a streaming fixture in the generated Rust tests.

use crate::e2e::codegen::streaming_assertions::RUST_STREAM_CRATE_PATH;

/// Wrap a stream-start expression so the test sees the error the stream reports.
///
/// `start` is the awaited call that returns `Result<Stream, E>`. The stream items are
/// `Result<Chunk, E>`. A start error and the first item error give the same `Result<(), E>`:
/// the fixture's error assertions then read one value, whichever side the error comes from.
/// When the stream ends with no item error, the value is `Ok(())` and `is_err()` fails the
/// test.
///
/// ~keep The wrapped value assumes the stream items share the error type of the start call.
/// The Rust streaming recipe makes the same assumption when it unwraps each item.
pub(super) fn read_stream_to_result(start: &str) -> String {
    format!(
        "match {start} {{\n        Ok(__stream) => match {RUST_STREAM_CRATE_PATH}StreamExt::collect::<Vec<_>>(__stream).await.into_iter().find_map(|__item| __item.err()) {{\n            Some(__stream_err) => Err(__stream_err),\n            None => Ok(()),\n        }},\n        Err(__start_err) => Err(__start_err),\n    }}"
    )
}
