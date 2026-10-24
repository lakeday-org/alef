#[unsafe(no_mangle)]
pub extern "C" fn fixture_version() -> *const std::ffi::c_char {
    c"1.0.0".as_ptr()
}
