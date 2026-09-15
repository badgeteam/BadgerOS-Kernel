use std::{fs::File, io::Write, process::Command};

fn main() {
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

    let mut bindings =
        bindgen::Builder::default().clang_args(["-Imisc/abi", "-Wno-unknown-attributes"]);

    for file in std::fs::read_dir("misc/abi/abi-bits/").expect("Failed to read misc/abi/") {
        let filename = file
            .expect("Failed to read dirent")
            .file_name()
            .into_string()
            .expect("Invalid filename");
        bindings = bindings.header(format!("misc/abi/abi-bits/{}", filename));
    }

    bindings
        .use_core()
        .generate()
        .expect("Unable to generate ABI bindings")
        .write_to_file("target/abi.rs")
        .expect("Couldn't write bindings!");

    println!("cargo::rerun-if-changed=misc/");
}
