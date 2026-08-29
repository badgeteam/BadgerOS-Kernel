/// Miscellaneous architecture-specific helpers.
pub trait ArchMisc {
    /// Return address offset from frame pointer in bytes.
    const FP_RA_OFFSET: isize;
    /// Frame link offset from frame pointer in bytes.
    const FP_LINK_OFFSET: isize;
    /// Get frame pointer for backtraces.
    extern "C" fn cur_frame_ptr() -> *const ();
}
