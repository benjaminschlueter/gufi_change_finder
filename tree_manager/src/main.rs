#![allow(non_snake_case)]

use std::io::{self, BufRead};
use std::process::exit;

use clap::Parser;

use highest_change_tree::*;

fn main() {
    let args = Args::parse();

    let FS_ROOT_PATH = args.root_scoutfs;
    let ENABLE_TRANSLATION = args.translate;

    eprintln!("Starting tree_manager");

    let stdin = io::stdin();
    let reader = stdin.lock();

    let mut tree = ChangeTree::new(TreeData {
        path: FS_ROOT_PATH.clone(),
        ino: 1,
    });

    // input lines should be formatted like path\t\0ino
    for line in reader.lines() {
        let line_vec: Vec<String> = line.unwrap().split("\t\0").map(|s| s.to_string()).collect();

        if line_vec.len() != 2 {
            panic!("malformed input: line does not contain exactly one null separator");
        }

        let path = &line_vec[0];
        let ino: u64 = line_vec[1].parse().unwrap();
        
        if marfs_pathman::filter_internal(path) {
            ChangeTree::add_path(
                &mut tree,
                TreeData {
                    path: path.to_string(),
                    ino,
                },
            );
        }
    }

    // Finished adding to tree

    let mut parent_list: Vec<OutputListData> = Vec::new();

    ChangeTree::parse_leaves(&tree, &mut parent_list, FS_ROOT_PATH.clone());

    for mut item in parent_list {
        // generate fuse path here to keep filtering/translation in one place
        if ENABLE_TRANSLATION {
            if item.tree_data.path == FS_ROOT_PATH {
                item.fuse_path = marfs_pathman::internal_to_user("/marfs", "");
            } else {
                item.fuse_path = marfs_pathman::internal_to_user(
                    "/marfs",
                    item.tree_data
                        .path
                        .strip_prefix(&format!("{FS_ROOT_PATH}/"))
                        .expect("failed to remove root path prefix"),
                );
            }
        } else {
            item.fuse_path = item.tree_data.path.clone();
        }

        println!(
            "{}\t\0{}\t\0{}",
            item.tree_data.path, item.tree_data.ino, item.fuse_path
        );
    }

    eprintln!("Finished tree_manager");
}

/// parent-finder
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Root of ScoutFS filesystem
    #[arg(short, long)]
    root_scoutfs: String,

    /// Enable path translation from filesystem internal path to user path. If disabled, output
    /// path and fuse_path will be the same.
    #[arg(short, long)]
    translate: bool,
}
