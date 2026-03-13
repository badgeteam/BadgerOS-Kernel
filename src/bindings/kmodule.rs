use crate::bindings::raw::kmodule_t;

unsafe impl Sync for kmodule_t {}

macro_rules! register_kmodule {
    ($name: ident, $mod_ver: expr, $init: expr) => {
        #[used]
        #[unsafe(link_section = ".kmodules")]
        static KMODULE_TABLE_ENTRY: &'static crate::bindings::raw::kmodule_t = {
            use ::core::ffi::CStr;
            use $crate::bindings::raw::*;
            let name = unsafe {
                CStr::from_bytes_with_nul_unchecked(concat!(stringify!($name), "\0").as_bytes())
            };
            &kmodule_t {
                min_abi: [
                    KMODULE_ABI_MAJ as u8,
                    KMODULE_ABI_MIN as u8,
                    KMODULE_ABI_PAT as u8,
                ],
                mod_ver: $mod_ver,
                name: name.as_ptr(),
                init: {
                    unsafe extern "C" fn init_wrapper() {
                        $init();
                    }
                    Some(init_wrapper)
                },
                deinit: None,
            }
        };
    };
}
