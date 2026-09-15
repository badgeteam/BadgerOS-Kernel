macro_rules! cmake_config {
    ($name: ident, true) => {
        pub const $name: bool = true;
    };
    ($name: ident, false) => {
        pub const $name: bool = false;
    };
    ($name: ident, $value: literal) => {
        pub const $name: i32 = $value;
    };
    ($name: ident, $value: ident) => {
        pub const $name: &'static str = "$value";
    };
}

cmake_config!(BOOT_PROTOCOL, limine);
cmake_config!(ENABLE_ACPI, true);
cmake_config!(ENABLE_DTB, true);
cmake_config!(STACK_SIZE, 262144);
cmake_config!(TICKS_PER_SEC, 500);
cmake_config!(MAX_CPUS, 32);
cmake_config!(LOAD_MEASURE_WINDOW, 200);
cmake_config!(PAGE_SIZE, 4096);
cmake_config!(KTEST, true);
