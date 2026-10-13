use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

// The probe observes the core call's active window across Python threads. ~keep
static ACTIVE: AtomicBool = AtomicBool::new(false);

pub struct Handle;

impl Handle {
    pub fn into_raw(self) -> *mut std::ffi::c_void {
        Box::into_raw(Box::new(1_u8)).cast()
    }
}

pub fn core_active() -> bool {
    ACTIVE.load(Ordering::SeqCst)
}

pub fn make_handle(_name: &str) -> Handle {
    ACTIVE.store(true, Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(150));
    ACTIVE.store(false, Ordering::SeqCst);
    Handle
}

pub fn get_handle(name: &str) -> std::io::Result<Handle> {
    if name == "bad" {
        return Err(std::io::Error::other("bad handle request"));
    }
    Ok(make_handle(name))
}

pub struct Registry;

impl Registry {
    pub fn get_handle(&self, name: &str) -> std::io::Result<Handle> {
        get_handle(name)
    }
}

pub fn empty_handle() -> Handle {
    make_handle("empty")
}
