use super::raw::process_t;

/// Raw process handle.
#[derive(Clone, Copy, PartialEq, Eq)]
struct RawProcess {
    inner: *mut process_t,
}
impl !Send for RawProcess {}
