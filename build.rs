use std::{path::PathBuf, process::Command};

fn main() {
    Command::new("make")
        .arg("-C")
        .arg("src/scoutwrap")
        .arg("clean")
        .status()
        .expect("failed to clean src/scoutwrap");

    Command::new("make")
        .arg("-C")
        .arg("src/scoutwrap")
        .status()
        .expect("failed to make src/scoutwrap");

    let bindings_path = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("bindings.rs");

    // ScoutFS
    let scoutfs_path = PathBuf::from("src/scoutfs")
        .canonicalize()
        .expect("Cannot canonicalize path");

    eprintln!(
        "Top level ScoutFS source directory: {}",
        scoutfs_path.display()
    );

    // ScoutFS is kernel code and does not provide libs; Need a user library wrapper for the ioctl

    let bindings = bindgen::Builder::default()
        .header("src/scoutwrap/scoutwrap.h")
        .clang_arg("-Isrc/scoutfs")
        .clang_arg("-I/usr/include/libxml2")
        .blocklist_item("^FP_.*$") // for some reason, FP_NAN, etc. are defined twice, so block them and use the libc variant
        .wrap_unsafe_ops(true)
        .generate()
        .expect("Failed to generate bindings for {header_path_str");

    bindings
        .write_to_file(bindings_path)
        .expect("Failed to write to bindings.tmp");

    let scoutwrap_lib_path = "src/scoutwrap";
    println!("cargo:rustc-link-search={}", scoutwrap_lib_path);
    println!("cargo:rustc-env=LD_LIBRARY_PATH={}", scoutwrap_lib_path);
    println!("cargo:rustc-link-lib=scoutwrap");
}
