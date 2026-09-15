fn main() {
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
