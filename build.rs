use std::{path::PathBuf, process::Command};

const SCOUTWRAP_PATH: &'static str = "src/scoutwrap";

fn main() {
    println!("cargo::rerun-if-changed={}/scoutwrap.c", SCOUTWRAP_PATH);
    println!("cargo::rerun-if-changed={}/scoutwrap.h", SCOUTWRAP_PATH);
    println!("cargo::rerun-if-changed={}/Makefile", SCOUTWRAP_PATH);

    Command::new("make")
        .arg("-C")
        .arg(SCOUTWRAP_PATH)
        .arg("clean")
        .status()
        .expect("failed to clean src/scoutwrap");

    Command::new("make")
        .arg("-C")
        .arg(SCOUTWRAP_PATH)
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
        .header(format!("{}/scoutwrap.h", SCOUTWRAP_PATH))
        .clang_arg("-Isrc/scoutfs")
        .clang_arg("-I/usr/include/libxml2")
        .blocklist_item("^FP_.*$") // for some reason, FP_NAN, etc. are defined twice, so block them and use the libc variant
        .wrap_unsafe_ops(true)
        .generate()
        .expect("Failed to generate bindings for {header_path_str");

    bindings
        .write_to_file(bindings_path)
        .expect("Failed to write to bindings.tmp");

    println!("cargo:rustc-link-search={}", SCOUTWRAP_PATH);
    println!("cargo:rustc-env=LD_LIBRARY_PATH={}", SCOUTWRAP_PATH);
    println!("cargo:rustc-link-lib=scoutwrap");
}
