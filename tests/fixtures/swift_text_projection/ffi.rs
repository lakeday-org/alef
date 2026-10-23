pub unsafe extern "C" fn sample_version() -> *const std::ffi::c_char {
    c"sample".as_ptr()
}
