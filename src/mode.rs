// Process-wide output mode; global because `Ty::java()` and the printers have no `Cx`.

use std::sync::atomic::{AtomicBool, Ordering};

static SIMPLE: AtomicBool = AtomicBool::new(false);
// Per file: `Result` erased to its `Ok` type, errors thrown (simple mode, see `lower::throws`).
static THROWS: AtomicBool = AtomicBool::new(false);

pub fn set_simple(on: bool) {
    SIMPLE.store(on, Ordering::Relaxed);
}

pub fn simple() -> bool {
    SIMPLE.load(Ordering::Relaxed)
}

pub fn set_throws(on: bool) {
    THROWS.store(on, Ordering::Relaxed);
}

pub fn throws() -> bool {
    THROWS.load(Ordering::Relaxed)
}
