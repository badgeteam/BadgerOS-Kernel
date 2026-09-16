use std::{env, fs::File, io::Write, path::PathBuf, process::Command};

fn main() {
    let target_cflags = match env::var("TARGET_CFLAGS") {
        Ok(fl) => fl.split_ascii_whitespace().map(String::from).collect(),
        Err(_) => vec![],
    };
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let mut fd = File::create("target/version.rs").expect("Failed to open target/version.rs");

    fd.write(b"pub const RELEASE: &'static str = \"").unwrap();
    if let Ok(out) = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        && out.status.success()
    {
        fd.write(out.stdout.trim_ascii()).unwrap();
    } else {
        fd.write(b"UNKNOWN").unwrap();
    }
    fd.write(b"\";\n").unwrap();

    fd.write(b"pub const VERSION: &'static str = \"").unwrap();
    fd.write(
        Command::new("date")
            .arg("+%Y-%m-%d %H:%M:%S %Z")
            .output()
            .unwrap()
            .stdout
            .trim_ascii(),
    )
    .unwrap();
    fd.write(b"\";\n").unwrap();

    if std::env::var("CARGO_FEATURE_ACPI").is_ok() {
        let mut cc = cc::Build::new();
        cc.files([
            "lib/uacpi/source/tables.c",
            "lib/uacpi/source/types.c",
            "lib/uacpi/source/uacpi.c",
            "lib/uacpi/source/utilities.c",
            "lib/uacpi/source/interpreter.c",
            "lib/uacpi/source/opcodes.c",
            "lib/uacpi/source/namespace.c",
            "lib/uacpi/source/stdlib.c",
            "lib/uacpi/source/shareable.c",
            "lib/uacpi/source/opregion.c",
            "lib/uacpi/source/default_handlers.c",
            "lib/uacpi/source/io.c",
            "lib/uacpi/source/notify.c",
            "lib/uacpi/source/sleep.c",
            "lib/uacpi/source/registers.c",
            "lib/uacpi/source/resources.c",
            "lib/uacpi/source/event.c",
            "lib/uacpi/source/mutex.c",
            "lib/uacpi/source/osi.c",
        ]);
        cc.flags(&target_cflags);
        cc.flag("-DUACPI_SIZED_FREES=1");
        cc.include("lib/uacpi/include");
        cc.compile("uacpi");

        bindgen::Builder::default()
            .header("misc/uacpi_wrapper.h")
            .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
            .clang_args(["-Ilib/uacpi/include", "-DUACPI_SIZED_FREES=1"])
            .clang_args(&target_cflags)
            .prepend_enum_name(false)
            .use_core()
            .generate()
            .expect("Unable to generate uACPI bindings")
            .write_to_file(out_dir.join("uacpi.rs"))
            .expect("Failed to write uACPI bindings file");
    }

    let mut abi = bindgen::Builder::default().clang_args(["-Imisc/abi", "-Wno-unknown-attributes"]);

    for file in std::fs::read_dir("misc/abi/abi-bits/").expect("Failed to read misc/abi/") {
        let filename = file
            .expect("Failed to read dirent")
            .file_name()
            .into_string()
            .expect("Invalid filename");
        abi = abi.header(format!("misc/abi/abi-bits/{}", filename));
    }

    abi.use_core()
        .generate()
        .expect("Unable to generate ABI bindings")
        .write_to_file(out_dir.join("abi.rs"))
        .expect("Failed to write ABI bindings file");

    println!("cargo::rerun-if-changed=misc/");
}
