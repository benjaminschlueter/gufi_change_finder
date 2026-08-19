use std::io::{self, BufRead};
use std::process::exit;

use highest_change_tree::*;

fn main() {
    eprintln!("Starting tree_manager");

    let stdin = io::stdin();
    let reader = stdin.lock();

    let FS_ROOT_PATH = String::from("/marfs/mdal-root2");
    let LOOP_VERBOSE = false;

    let mut tree = ChangeTree::new(TreeData {
        path: FS_ROOT_PATH.clone(),
        ino: 1,
    });

    // input lines should be formatted like path\x00ino
    for line in reader.lines() {
        let line_vec: Vec<String> = line.unwrap().split("\x00").map(|s| s.to_string()).collect();

        if line_vec.len() != 2 {
            panic!("malformed input: line does not contain exactly one null separator");
        }

        let path = &line_vec[0];
        let ino: i64 = line_vec[1].parse().unwrap();

        eprintln!("path: {path}");
        eprintln!("ino: {ino}");

        // handle root directory separately because it has empty path
        // - execution continues after to advance to final state
        if ino == 1 {
            if LOOP_VERBOSE {
                println!("INFO\tfilesystem root detected: trimming all nodes below");
            }

            ChangeTree::trim_below_root(&mut tree);

            // no need to add anything else to the tree, just output the root
            println!("{}", FS_ROOT_PATH);
            eprintln!("Finished tree_manager");
            exit(0);
        }
     
    }

    eprintln!("Finished tree_manager");
}
