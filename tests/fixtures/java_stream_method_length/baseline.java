final class SampleClient {
    public java.util.stream.Stream<StreamItem> watch(final StreamRequest req) throws TestLibRsException {
        java.util.Objects.requireNonNull(req, "req must not be null");
        final MemorySegment streamHandle;
        try (var arena = Arena.ofShared()) {
            String requestJson = STREAM_MAPPER.writeValueAsString(req);
            var cRequestJson = arena.allocateFrom(requestJson);
            MemorySegment requestPtr = (MemorySegment) NativeLib.TL_STREAM_REQUEST_FROM_JSON.invoke(cRequestJson);
            if (requestPtr.equals(MemorySegment.NULL)) {
                checkLastFfiError();
                throw new TestLibRsException("watch: failed to marshal request", (Throwable) null);
            }
            Throwable startFailure = null;
            try (HandleLease handleLease = borrowHandle()) {
                streamHandle = (MemorySegment) NativeLib.TL_SAMPLE_CLIENT_WATCH_START.invoke(handleLease.handle(), requestPtr);
                if (streamHandle == null || streamHandle.equals(MemorySegment.NULL)) {
                    checkLastFfiError();
                    throw new TestLibRsException("watch: stream handle was null", (Throwable) null);
                }
            } catch (Throwable failure) {
                startFailure = failure;
                throw failure;
            } finally {
                try {
                    NativeLib.TL_STREAM_REQUEST_FREE.invoke(requestPtr);
                } catch (Throwable cleanupFailure) {
                    if (startFailure != null) startFailure.addSuppressed(cleanupFailure);
                    else throw cleanupFailure;
                }
            }
        } catch (Throwable e) {
            if (e instanceof TestLibRsException ex) { throw ex; }
            throw new TestLibRsException("watch: failed to start stream", e);
        }
        final MemorySegment finalStreamHandle = streamHandle;
        final class StreamingIterator implements java.util.Iterator<StreamItem>, AutoCloseable {
            private boolean closed = false;
            private Throwable deferred = null;
            private StreamItem pending = pull();

            /** Throws a typed error from an iterator method that cannot declare it. */
            @SuppressWarnings("unchecked")
            private static <T extends Throwable> RuntimeException rethrow(final Throwable error) throws T {
                throw (T) error;
            }

            private TestLibRsException failure(String message, Throwable cause) {
                return new TestLibRsException("watch: " + message, cause);
            }

            private void cleanup(Throwable primary, MemorySegment jsonPtr, MemorySegment chunkPtr) {
                Throwable cleanupError = null;
                if (jsonPtr != null && !jsonPtr.equals(MemorySegment.NULL)) {
                    try { NativeLib.TL_FREE_STRING.invoke(jsonPtr); }
                    catch (Throwable error) { cleanupError = error; }
                }
                if (chunkPtr != null && !chunkPtr.equals(MemorySegment.NULL)) {
                    try { NativeLib.TL_STREAM_ITEM_FREE.invoke(chunkPtr); }
                    catch (Throwable error) {
                        if (cleanupError == null) { cleanupError = error; }
                        else { cleanupError.addSuppressed(error); }
                    }
                }
                if (cleanupError == null) { return; }
                if (primary != null) { primary.addSuppressed(cleanupError); }
                else {
                    TestLibRsException cleanupPrimary = failure("failed to clean stream item", cleanupError);
                    close(cleanupPrimary);
                    throw rethrow(cleanupPrimary);
                }
            }

            private StreamItem pull() {
                if (closed) { return null; }
                MemorySegment chunkPtr;
                try {
                    chunkPtr = (MemorySegment) NativeLib.TL_SAMPLE_CLIENT_WATCH_NEXT.invoke(finalStreamHandle);
                } catch (Throwable error) {
                    TestLibRsException primary = failure("stream advance failed", error);
                    close(primary);
                    throw rethrow(primary);
                }
                if (chunkPtr.equals(MemorySegment.NULL)) {
                    Throwable primary = null;
                    try {
                        int code = (int)(long) NativeLib.TL_LAST_ERROR_CODE.invoke();
                        if (code != 0) {
                            MemorySegment ctxPtr = (MemorySegment) NativeLib.TL_LAST_ERROR_CONTEXT.invoke();
                            String msg = ctxPtr.equals(MemorySegment.NULL)
                                ? "Native call failed with error code " + code
                                : ctxPtr.reinterpret(Long.MAX_VALUE).getString(0);
                            primary = NativeLib.lastErrorException(code, msg);
                        }
                    } catch (Throwable error) {
                        primary = failure("failed to read stream status", error);
                    }
                    close(primary);
                    if (primary != null) { throw rethrow(primary); }
                    return null;
                }
                MemorySegment jsonPtr = MemorySegment.NULL;
                Throwable primary = null;
                try {
                    jsonPtr = (MemorySegment) NativeLib.TL_STREAM_ITEM_TO_JSON.invoke(chunkPtr);
                    if (jsonPtr.equals(MemorySegment.NULL)) {
                        throw failure("failed to serialize chunk", null);
                    }
                    String json = jsonPtr.reinterpret(Long.MAX_VALUE).getString(0);
                    return STREAM_MAPPER.readValue(json, StreamItem.class);
                } catch (Throwable error) {
                    primary = error instanceof TestLibRsException || error instanceof RuntimeException
                        ? error
                        : failure("failed to deserialize chunk", error);
                    close(primary);
                    throw rethrow(primary);
                } finally {
                    cleanup(primary, jsonPtr, chunkPtr);
                }
            }

            private void close(Throwable primary) {
                if (closed) { return; }
                closed = true;
                try { NativeLib.TL_SAMPLE_CLIENT_WATCH_FREE.invoke(finalStreamHandle); }
                catch (Throwable cleanupError) {
                    if (primary != null) { primary.addSuppressed(cleanupError); }
                    else { throw rethrow(failure("failed to free stream", cleanupError)); }
                }
            }

            @Override
            public void close() { close(null); }

            @Override
            public boolean hasNext() { return pending != null || deferred != null; }

            @Override
            public StreamItem next() {
                if (pending == null) {
                    Throwable failed = deferred;
                    if (failed == null) { throw new java.util.NoSuchElementException(); }
                    deferred = null;
                    throw rethrow(failed);
                }
                StreamItem current = pending;
                // The held chunk is already complete: return it now and report a failed read
                // on the following call.
                try { pending = pull(); }
                catch (Exception error) { pending = null; deferred = error; }
                return current;
            }
        }
        StreamingIterator iterator = new StreamingIterator();
        return java.util.stream.StreamSupport.stream(
            java.util.Spliterators.spliteratorUnknownSize(
                iterator,
                java.util.Spliterator.ORDERED | java.util.Spliterator.NONNULL
            ),
            false
        ).onClose(iterator::close);
    }
}
