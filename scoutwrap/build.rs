use std::{path::PathBuf, process::Command};

const WORKSPACE_PATH: &'static str = "/opt/storage/gufi_change_finder";
const SCOUTWRAP_PATH: &'static str = "src/scoutwrap";
const SCOUTFS_PATH: &'static str = "src/scoutfs";

fn main() {
    println!("cargo::rerun-if-changed={}/scoutwrap.c", SCOUTWRAP_PATH);
    println!("cargo::rerun-if-changed={}/scoutwrap.h", SCOUTWRAP_PATH);
    println!("cargo::rerun-if-changed={}/scoutwrap.a", SCOUTWRAP_PATH);
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

    // ScoutFS is kernel code and does not provide libs; Need a user library wrapper for the ioctl

    let bindings = bindgen::Builder::default()
        .header(format!("{}/scoutwrap.h", SCOUTWRAP_PATH))
        .clang_arg(format!("-I{}", SCOUTFS_PATH))
        .allowlist_type("scoutfs_ioctl_walk_inodes")
        .allowlist_type("scoutfs_ioctl_walk_inodes_entry")
        .allowlist_function("wrap_walk_inodes")
        .allowlist_type("scoutfs_ioctl_ino_path")
        .allowlist_type("scoutfs_ioctl_ino_path_result")
        .allowlist_function("wrap_ino_path")
        .allowlist_type("scoutfs_ioctl_listxattr_hidden")
        .allowlist_function("wrap_listxattr_hidden")
        .generate()
        .expect("Failed to generate bindings");

    bindings
        .write_to_file(bindings_path)
        .expect("Failed to write to {bindings_path}");

    println!("cargo:rustc-link-search={}/scoutwrap/{}", WORKSPACE_PATH, SCOUTWRAP_PATH);
    println!("cargo:rustc-env=LD_LIBRARY_PATH={}", SCOUTWRAP_PATH);
    println!("cargo:rustc-link-lib=scoutwrap");
}
