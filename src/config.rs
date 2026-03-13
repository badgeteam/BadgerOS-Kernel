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

include!("../build/cmake_config.rs");
