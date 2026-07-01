use std::path::PathBuf;
use std::fs::File;
use std::io::Write;
use std::process::Command;

fn main() {
    let mut log_file = File::create("build.log").expect("Failed to create build.log file");
	
    let top_path = PathBuf::from(".").canonicalize().expect("failed to canonicalize top level path");

    log_file.write_all(b"Top level MarFS source directory: ").expect("Failed to write to build.log");
    log_file.write_all(b"\n").expect("Failed to write to build.log");

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
    let scoutfs_path = PathBuf::from("src/scoutfs").canonicalize().expect("Cannot canonicalize path");
    
    log_file.write_all(b"Top level ScoutFS source directory: ").expect("Failed to write to build.log");
    log_file.write_all(scoutfs_path.clone().into_os_string().as_encoded_bytes()).expect("Failed to write to build.log");
    log_file.write_all(b"\n").expect("Failed to write to build.log");

    // ScoutFS headers

    let scoutwrap_path = top_path.join("src/scoutwrap/scoutwrap.h");
    let scoutwrap_path_str = scoutwrap_path.to_str().expect("Failed to convert header path to String");

    // ScoutFS is kernel code and does not provide libs; Need a user library wrapper for the ioctl
    
    log_file.write_all(b"Searching for libs in: ").expect("Failed to write to build.log");
    log_file.write_all(b"\n\n").expect("Failed to write to build.log");
    
    let bindings = bindgen::Builder::default()
                    .header(scoutwrap_path_str)
                    .clang_arg(format!("-I{}/src/scoutfs", top_path.to_str().unwrap()))
                    .clang_arg("-I/usr/include/libxml2")
                    .blocklist_item("^FP_.*$") // for some reason, FP_NAN, etc. are defined twice, so block them and use the libc variant
                    .wrap_unsafe_ops(true)
                    .generate()
                    .expect("Failed to generate bindings for {header_path_str");
    
    bindings.write_to_file(bindings_path).expect("Failed to write to bindings.tmp");

    //let log_string = format!("Wrote bindings from {header_path_str} to {bindings_path}\n"); 
    //log_file.write_all(log_string.as_bytes()).expect("Failed to write to build.log");

    let scoutwrap_lib_path = top_path.join("src/scoutwrap");
    
    println!("cargo:rustc-link-search={}", scoutwrap_lib_path.to_str().unwrap());
    println!("cargo:rustc-env=LD_LIBRARY_PATH={}", scoutwrap_lib_path.to_str().unwrap());
    println!("cargo:rustc-link-lib=scoutwrap");

}
