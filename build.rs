use std::{
    fs,
    io::{Read, Write},
    process::Command,
};

fn main() {
    let mut release = [0u8; 64];
    let version_len = fs::File::open("VERSION")
        .expect("Can't open VERSION file")
        .read(&mut release)
        .expect("Failed to read version");
    let release = str::from_utf8(&release[..version_len]).unwrap();

    let output = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .unwrap();

    let tag;
    if output.status.success() {
        tag = output.stdout;
    } else {
        tag = Vec::from(b"(no commit)");
    }
    let tag = str::from_utf8(&tag).unwrap();

    let date = Command::new("date")
        .args(["+%Y-%m-%d %H:%M:%S %Z"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "(undated)".into());

    let version = format!("{} {}", tag.trim_ascii(), date.trim_ascii());

    let verfile = fs::File::create("target/version.rs").expect("Can't create version file");
    write!(
        &verfile,
        "
pub const RELEASE: &'static str = {:?};
pub const VERSION: &'static str = {:?};
",
        release, version
    )
    .unwrap();

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        .clang_args([
            "-Iinclude",
            "-Iinclude/badgelib",
            "-Iport/generic/include",
            "-Icpu/riscv64/include",
            "-I../common/include",
            "-I../common/badgelib/include",
            "-Wno-unknown-attributes",
            "-DBADGEROS_KERNEL",
        ])
        // Fix: For some reason, `malloc` and `realloc` specifically do not use `usize`.
        .blocklist_function("malloc")
        .blocklist_function("realloc")
        .blocklist_function("calloc")
        .raw_line(
            "use core::ffi::{c_char, c_void};
unsafe extern \"C\" {
    pub fn malloc(_: usize) -> *mut c_void;
    pub fn calloc(_: usize, _: usize) -> *mut c_void;
    pub fn realloc(_: *mut c_void, _: usize) -> *mut c_void;
    pub fn memset(dest: *mut c_void, val: u8, size: usize);
    pub fn memcpy(dest: *mut c_void, src: *const c_void, size: usize);
    pub fn strlen(cstr: *const c_char) -> usize;
}",
        )
        // The input header we would like to generate
        // bindings for.
        .header("include/rust_bindgen_wrapper.h")
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Use core, not std.
        .use_core()
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    bindings
        .write_to_file("target/bindings.rs")
        .expect("Couldn't write bindings!");
}
