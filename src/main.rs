#![allow(non_snake_case)]

mod scoutwrap;
use scoutwrap::*;

mod highest_change_tree;
use highest_change_tree::*;

use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use clap::Parser;

fn main() {
    // abort if not root
    if users::get_current_uid() != 0 {
        panic!("Must run as root!");
    }

    let args = Args::parse();

    let CHECKPOINT_MS = args.checkpoint_ms;
    let BATCH_SIZE = args.batch_size;
    let STATE_FILE = args.state_file_path;
    let STATE_SWAP_FILE = format!("{STATE_FILE}.swp");
    let STATE_VERBOSE = args.state_verbose;
    let LOOP_VERBOSE = args.loop_verbose;
    let FS_ROOT_PATH = args.root_scoutfs;
    let OUTPUT_DIR = args.output_file_dir;
    let QUOTA_STATE_FILE = args.quota_state_file_path;

    let mut starting_major: i64 = 0;
    let mut starting_ino: i64 = 0;
    let mut starting_minor: i64 = 0;

    // if state file does not exist, create it and start from 0. On all other errors, panic.
    match OpenOptions::new().read(true).open(&STATE_FILE) {
        Ok(f) => {
            let mut reader = BufReader::new(&f);
            let mut starting_state_str = String::new();

            if let Err(e) = reader.read_to_string(&mut starting_state_str) {
                panic!("read_to_string: {}", e.to_string());
            }

            let input_vec: Vec<String> = starting_state_str
                .split("\n")
                .map(|s| s.to_string())
                .collect();

            starting_major = input_vec[0]
                .trim()
                .parse()
                .expect("state file does not contain valid integer");
            starting_ino = input_vec[1]
                .trim()
                .parse()
                .expect("state file does not contain valid integer");
            starting_minor = input_vec[2]
                .trim()
                .parse()
                .expect("state file does not contain valid integer");
        }
        Err(e) => {
            if e.kind() == ErrorKind::NotFound {
                if STATE_VERBOSE {
                    println!("No state file found: starting at initial state 0");
                }
            } else {
                panic!("open: {}\nFailed to open state file", e.to_string());
            }
        }
    }

    // check for existing STATE_SWAP_FILE
    if let Ok(_) = OpenOptions::new().read(true).open(&STATE_SWAP_FILE) {
        if STATE_VERBOSE {
            println!("Detected state swp file... removing")
        }

        if let Err(e) = std::fs::remove_file(Path::new(&STATE_SWAP_FILE)) {
            panic!("failed to remove swp state file: {e}");
        }
    }

    let quota_major: i64;
    let quota_ino: i64;
    let quota_minor: i64;

    // read state info from quota to keep tools in sync
    match OpenOptions::new().read(true).open(&QUOTA_STATE_FILE) {
        Ok(f) => {
            let mut reader = BufReader::new(&f);
            let mut starting_state_str = String::new();

            if let Err(e) = reader.read_to_string(&mut starting_state_str) {
                panic!("read_to_string: {}", e.to_string());
            }

            let input_vec: Vec<String> = starting_state_str
                .split("\n")
                .map(|s| s.to_string())
                .collect();

            quota_major = input_vec[0]
                .trim()
                .parse()
                .expect("quota state file does not contain valid integer");
            quota_ino = input_vec[1]
                .trim()
                .parse()
                .expect("quota state file does not contain valid integer");
            quota_minor = input_vec[2]
                .trim()
                .parse()
                .expect("quota state file does not contain valid integer");
        }
        Err(e) => {
            // fatal if no quota state available
            panic!("open: {}\nFailed to open quota state file", e.to_string());
        }
    }

    if STATE_VERBOSE {
        println!("INFO\tdetected quota state ({quota_major}, {quota_ino}, {quota_minor})");
    }

    // open fd for filesystem root
    let fs_root;
    match OpenOptions::new().read(true).open(&FS_ROOT_PATH) {
        Ok(f) => fs_root = f,
        Err(e) => panic!(
            "open: {}\nFailed to open filesystem root at {}",
            e, &FS_ROOT_PATH
        ),
    }

    // setup walk_inodes struct

    let first = ScoutwrapWalkInodesEntry {
        major: starting_major as u64,
        ino: starting_ino as u64,
        minor: starting_minor as u32,
    };

    let last = ScoutwrapWalkInodesEntry {
        major: std::u64::MAX,
        ino: std::u64::MAX,
        minor: std::u32::MAX,
    };

    let mut walk_inodes_arg = ScoutwrapWalkInodes {
        first: first,
        last: last,
        entries_vec: Vec::new(),
        nr_entries: BATCH_SIZE,
        index: 0,
    };

    let mut final_major = 0;
    let mut final_ino = 0;
    let mut final_minor = 0;

    // create HashMap and tree
    let mut arena = highest_change_tree_create();

    // add root node
    let tree_root = highest_change_tree_new_node(
        &mut arena,
        TreeData {
            name: FS_ROOT_PATH.clone(),
            ino: 1,
        },
    );

    let mut root_scanned = false;

    if STATE_VERBOSE {
        println!(
            "Running gufi_change_finder with starting state (major: {}, ino: {}, minor: {})",
            starting_major, starting_ino, starting_minor
        );
    }

    let start_time = Instant::now();
    let mut last_checkpoint = Duration::from_millis(0);

    // process batches until entries vector is empty
    loop {
        match scoutwrap_walk_inodes(&fs_root, walk_inodes_arg.clone()) {
            Ok(w) => walk_inodes_arg = w,
            Err(e) => {
                panic!("scoutwrap_walk_inodes: {}", e);
            }
        }

        // batch vector will never be empty: last element always part of next for full batches and non-full batches will be the last batch

        let mut last_batch = false;
        if walk_inodes_arg.entries_vec.len() < BATCH_SIZE {
            last_batch = true;
        }

        // process all but last element: last will be starting point of next run
        for entry in &walk_inodes_arg.entries_vec {
            // don't process the last entry of batches that are not the last batch. The last entry of the final batch will be processed.
            if !last_batch && entry.ino == walk_inodes_arg.entries_vec.last().unwrap().ino {
                walk_inodes_arg.first.major = walk_inodes_arg.entries_vec.last().unwrap().major;
                walk_inodes_arg.first.ino = walk_inodes_arg.entries_vec.last().unwrap().ino;
                walk_inodes_arg.first.minor = walk_inodes_arg.entries_vec.last().unwrap().minor;
                break;
            }

            let major = entry.major;
            let ino = entry.ino;
            let minor = entry.minor;

            // skip entry if it matches the starting values: it was processed in the last execution
            if major == starting_major as u64
                && ino == starting_ino as u64
                && minor == starting_minor as u32
            {
                if LOOP_VERBOSE {
                    println!("skipping starting value");
                }

                continue;
            }

            // stop if we are going to get ahead of quota_update
            if major >= quota_major as u64 && minor >= quota_minor as u32 {
                if STATE_VERBOSE || LOOP_VERBOSE {
                    println!("INFO\treached quota state: stopping here");
                }

                last_batch = true;

                break;
            }

            let mut ino_path_arg = ScoutwrapInoPath {
                ino: ino,
                dir_ino: 0,
                dir_pos: 0,
                result_ptr: 0,
                result_bytes: STR_BUF_SIZE,
            };

            // call ino_path ioctl until all paths to inode are found

            let mut ino_path_vec = Vec::new();
            loop {
                match scoutwrap_ino_path(&fs_root, ino_path_arg.clone()) {
                    Ok(p) => {
                        ino_path_vec.push(p.path);

                        // based on ioctl usage from scoutfs/utils/src/ino_path.c:64
                        ino_path_arg.dir_ino = p.dir_ino;
                        ino_path_arg.dir_pos = p.dir_pos;

                        ino_path_arg.dir_pos += 1;
                        if ino_path_arg.dir_pos == 0 {
                            ino_path_arg.dir_ino += 1;
                            if ino_path_arg.dir_ino == 0 {
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        if std::io::Error::last_os_error().kind() == ErrorKind::NotFound {
                            // handle a case where a deleted files inode will still show up in the changelog
                            // this case now happens every inode on the final loop iteration
                            break;
                        } else {
                            panic!("scoutwrap_ino_path: {} on inode {}", e, ino);
                        }
                    }
                }
            }

            // handle root directory separately because it has empty path
            // - execution continues after to advance to final state
            if ino == 1 {
                if LOOP_VERBOSE {
                    println!("INFO\tfilesystem root detected: trimming all nodes below");
                }

                highest_change_tree_trim_below(&mut arena, tree_root);

                root_scanned = true;

                final_major = major;
                final_ino = ino;
                final_minor = minor;

                continue;
            }

            // if root has been scanned, update state values and move on to minimize work
            if root_scanned {
                final_major = major;
                final_ino = ino;
                final_minor = minor;

                continue;
            }

            // handle all paths to inode from hard links
            for path in ino_path_vec {
                if LOOP_VERBOSE {
                    println!("INFO\tprocessing\tinode: {}\tpath: {}", ino, path);
                }

                highest_change_tree_add_path(&mut arena, tree_root, path, ino);

                // set final state to the last file processed. This means the last file will be processed again in the next run, but this tool is idempotent.

                final_major = major;
                final_ino = ino;
                final_minor = minor;
            } // end ino_path_vec loop
        }

        let cur_time = start_time.elapsed();

        // save state on last batch or every CHECKPOINT_MS
        if last_batch || cur_time - last_checkpoint > Duration::from_millis(CHECKPOINT_MS) {
            if LOOP_VERBOSE || STATE_VERBOSE {
                println!("INFO\tcheckpoint at {:?}", cur_time);
            }

            // update state file with final state
            if final_major != starting_major as u64 && final_major != 0 {
                let mut new_state_file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(&STATE_SWAP_FILE)
                    .expect("failed to open state swp file");

                let write_str = format!(
                    "{}\n{}\n{}",
                    final_major.to_string(),
                    final_ino.to_string(),
                    final_minor.to_string()
                );

                if let Err(e) = new_state_file.write_all(write_str.as_bytes()) {
                    panic!("failed to write new state: {}", e.to_string());
                }

                if let Err(e) = std::fs::rename(&STATE_SWAP_FILE, &STATE_FILE) {
                    panic!("failed to rename state swp file: {}", e.to_string())
                }
            }

            last_checkpoint = cur_time;
        }

        if last_batch {
            break;
        }
    }

    // don't bother with a parent list and output file if nothing changed

    if final_major == 0 && final_ino == 0 && final_minor == 0 {
        if STATE_VERBOSE {
            println!("INFO\tno changes detected");
            println!(
                "Finished gufi_change_finder at final state (major: {}, ino: {}, minor: {})",
                starting_major, starting_ino, starting_minor
            );
        }

        return;
    }

    let mut parent_list: Vec<TreeData> = Vec::new();

    if root_scanned {
        parent_list.push(TreeData {
            name: FS_ROOT_PATH.clone(),
            ino: 1,
        });
    }

    highest_change_tree_parse_leaves(tree_root, &mut parent_list, FS_ROOT_PATH, &arena);

    // write output file if entries were processed

    std::fs::create_dir_all(&OUTPUT_DIR).expect("failed to create output directory");

    let output_file_name =
        format!("{OUTPUT_DIR}/gufi_change_finder.out.{final_major}.{final_ino}.{final_minor}");
    let output_file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(output_file_name)
        .expect("failed to open output file");

    let mut writer = BufWriter::new(output_file);

    for item in &parent_list {
        // terminate with null byte to protect issues with weird user paths
        writeln!(writer, "{}\0,{}", item.name, item.ino)
            .expect("failed to write path to output file");
    }

    // debug print tree for development
    println!("{:?}", parent_list);

    if STATE_VERBOSE {
        println!(
            "Finished gufi_change_finder at final state (major: {}, ino: {}, minor: {})",
            final_major, final_ino, final_minor
        );
    }
}

/// parent-finder
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Frequency of checkpoints to the state file
    #[arg(short = 'm', long, default_value_t = 60000)]
    checkpoint_ms: u64,

    /// Number of inodes to process in a single batch
    #[arg(short, long, default_value_t = 65536)]
    batch_size: usize,

    /// Print info on start/final state and state file existence [default: true]
    #[arg(short, long)]
    state_verbose: bool,

    /// Print details for each processing step for each file. For debugging purposes (lots of output) [default: false]
    #[arg(short, long)]
    loop_verbose: bool,

    /// State file path (and state swap file)
    #[arg(short = 'p', long, default_value_t = String::from(".state"))]
    state_file_path: String,

    /// Root of ScoutFS filesystem
    #[arg(short, long)]
    root_scoutfs: String,

    /// Parent directory of output files
    #[arg(short, long, default_value_t = String::from("./output"))]
    output_file_dir: String,

    /// Quota state file path
    #[arg(short, long)]
    quota_state_file_path: String,
}
