#![allow(non_snake_case)]

use scoutwrap::*;

use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::io::{BufReader, Read, Write};
use std::path::Path;

use clap::Parser;

fn main() {
    // abort if not root
    if users::get_current_uid() != 0 {
        panic!("Must run as root!");
    }

    let args = Args::parse();

    let BATCH_SIZE = args.batch_size;
    let STATE_FILE = args.state_file_path;
    let STATE_SWAP_FILE = format!("{STATE_FILE}.swp");
    let STATE_VERBOSE = args.state_verbose;
    let LOOP_VERBOSE = args.loop_verbose;
    let FS_ROOT_PATH = args.root_scoutfs;
    let QUOTA_STATE_FILE = args.quota_state_file_path;

    let mut starting_state = WalkInodesEntry {
        major: 0,
        ino: 0,
        minor: 0,
    };

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

            starting_state.major = input_vec[0]
                .trim()
                .parse()
                .expect("state file does not contain valid integer");
            starting_state.ino = input_vec[1]
                .trim()
                .parse()
                .expect("state file does not contain valid integer");
            starting_state.minor = input_vec[2]
                .trim()
                .parse()
                .expect("state file does not contain valid integer");
        }
        Err(e) => {
            if e.kind() == ErrorKind::NotFound {
                if STATE_VERBOSE {
                    eprintln!("No state file found: starting at initial state 0");
                }
            } else {
                panic!("open: {}\nFailed to open state file", e.to_string());
            }
        }
    }

    // check for existing STATE_SWAP_FILE
    if let Ok(_) = OpenOptions::new().read(true).open(&STATE_SWAP_FILE) {
        if STATE_VERBOSE {
            eprintln!("Detected state swp file... removing")
        }

        if let Err(e) = std::fs::remove_file(Path::new(&STATE_SWAP_FILE)) {
            panic!("failed to remove swp state file: {e}");
        }
    }
    let mut quota_state = WalkInodesEntry {
        major: 0,
        ino: 0,
        minor: 0,
    };

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

            quota_state.major = input_vec[0]
                .trim()
                .parse()
                .expect("quota state file does not contain valid integer");
            quota_state.ino = input_vec[1]
                .trim()
                .parse()
                .expect("quota state file does not contain valid integer");
            quota_state.minor = input_vec[2]
                .trim()
                .parse()
                .expect("quota state file does not contain valid integer");
        }
        Err(e) => {
            // fatal if no quota state available
            panic!("open: {}\nFailed to open quota state file", e.to_string());
        }
    }

    eprintln!(
        "INFO\tdetected quota state ({}, {}, {})",
        quota_state.major, quota_state.ino, quota_state.minor
    );

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

    let first = scoutwrap::WalkInodesEntry {
        major: starting_state.major as u64,
        ino: starting_state.ino as u64,
        minor: starting_state.minor as u32,
    };

    let last = scoutwrap::WalkInodesEntry {
        major: std::u64::MAX,
        ino: std::u64::MAX,
        minor: std::u32::MAX,
    };

    let mut walk_inodes_arg = scoutwrap::WalkInodes {
        first: first,
        last: last,
        entries_vec: Vec::new(),
        nr_entries: BATCH_SIZE,
        index: 0,
    };

    let mut final_state = WalkInodesEntry {
        major: 0,
        ino: 0,
        minor: 0,
    };

    eprintln!(
        "Starting parse_changelog with starting state (major: {}, ino: {}, minor: {})",
        starting_state.major, starting_state.ino, starting_state.minor
    );

    // process batches until entries vector is empty
    loop {
        match scoutwrap::walk_inodes(&fs_root, walk_inodes_arg.clone()) {
            Ok(w) => walk_inodes_arg = w,
            Err(e) => {
                panic!("scoutwrap::walk_inodes: {}", e);
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

            // skip entry if it matches the starting values: it was processed in the last execution
            if entry.major == starting_state.major as u64
                && entry.ino == starting_state.ino as u64
                && entry.minor == starting_state.minor as u32
            {
                if LOOP_VERBOSE {
                    eprintln!("skipping starting value");
                }

                continue;
            }

            // stop if we are going to get ahead of quota_update
            if entry.major >= quota_state.major as u64 && entry.minor >= quota_state.minor as u32 {
                eprintln!("INFO\treached quota state: stopping here");

                last_batch = true;

                break;
            }

            let mut ino_path_arg = scoutwrap::InoPath {
                ino: entry.ino,
                dir_ino: 0,
                dir_pos: 0,
                result_ptr: 0,
                result_bytes: STR_BUF_SIZE,
            };

            // call ino_path ioctl until all paths to inode are found

            let mut ino_path_vec = Vec::new();
            loop {
                match scoutwrap::ino_path(&fs_root, ino_path_arg.clone()) {
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
                            panic!("scoutwrap::ino_path: {} on inode {}", e, entry.ino);
                        }
                    }
                }
            }

            // handle all paths to inode from hard links
            for path in ino_path_vec {
                if LOOP_VERBOSE {
                    eprintln!("INFO\tprocessing\tinode: {}\tpath: {}", entry.ino, path);
                }

                // print paths and inodesto stdout and all logs to stderr
                println!("{}\t\0{}", path, entry.ino);

                // set final state to the last file processed. This means the last file will be processed again in the next run, but this tool is idempotent.

                /*
                final_state.major = entry.major;
                final_state.ino = entry.ino;
                final_state.minor = entry.minor;
                */

                final_state = entry.clone();
            } // end ino_path_vec loop
        }

        // save state on last batch
        if last_batch {
            // update state file with final state
            if final_state.major != starting_state.major as u64 && final_state.major != 0 {
                let mut new_state_file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .open(&STATE_SWAP_FILE)
                    .expect("failed to open state swp file");

                let write_str = format!(
                    "{}\n{}\n{}",
                    final_state.major.to_string(),
                    final_state.ino.to_string(),
                    final_state.minor.to_string()
                );

                if let Err(e) = new_state_file.write_all(write_str.as_bytes()) {
                    panic!("failed to write new state: {}", e.to_string());
                }

                if let Err(e) = std::fs::rename(&STATE_SWAP_FILE, &STATE_FILE) {
                    panic!("failed to rename state swp file: {}", e.to_string())
                }
            }

            break;
        }
    }

    if final_state.major == 0 && final_state.ino == 0 && final_state.minor == 0 {
        eprintln!("INFO\tno changes detected");
        eprintln!(
            "Finished parse_changelog at final state (major: {}, ino: {}, minor: {})",
            starting_state.major, starting_state.ino, starting_state.minor
        );
    } else {
        eprintln!(
            "Finished parse_changelog at final state (major: {}, ino: {}, minor: {})",
            final_state.major, final_state.ino, final_state.minor
        );
    }
}

/// parent-finder
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
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
