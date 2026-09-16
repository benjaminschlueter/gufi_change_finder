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
    let VERBOSE = args.verbose;
    let FS_ROOT_PATH = args.root_scoutfs;
    let QUOTA_STATE_FILE = args.quota_state_file_path;
    let VALIDATE_REINDEX = args.validate_reindex;
    let VALIDATE_REINDEX_PATH = if VALIDATE_REINDEX {
        format!("{STATE_FILE}.reindex_validate")
    } else {
        String::new()
    };

    let mut starting_state;
    match read_state_from_file(&STATE_FILE) {
        Ok(s) => starting_state = s,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("INFO\tstarting from state 0");
            starting_state = WalkInodesEntry {
                major: 0,
                ino: 0,
                minor: 0,
            };
        }
    }

    // check for existing STATE_SWAP_FILE
    if OpenOptions::new().read(true).open(&STATE_SWAP_FILE).is_ok() {
        eprintln!("INFO\tdetected state swp file... removing");

        if let Err(e) = std::fs::remove_file(Path::new(&STATE_SWAP_FILE)) {
            panic!("failed to remove swp state file: {e}");
        }
    }

    // if this is enabled, parse_changelog will read a statefile output by reindexer.py to confirm
    // reindexer finished properly before proceeding further into the changelog
    if VALIDATE_REINDEX {
        match read_state_from_file(&VALIDATE_REINDEX_PATH) {
            Ok(s) => {
                let reindex_validate_state = s;
                if reindex_validate_state != starting_state {
                    eprintln!("WARNING\treindexer validated state and starting state do not match");
                    eprintln!("WARNING\trestarting from last validated state");
                    starting_state = reindex_validate_state;
                }
            }
            Err(e) => {
                panic!("failed to open reindex validation file: {e}");
            }
        }
    }

    let quota_state = match read_state_from_file(&QUOTA_STATE_FILE) {
        Ok(s) => s,
        Err(e) => {
            panic!("failed to open quota state file: {e}");
        }
    };

    eprintln!(
        "INFO\tdetected quota state ({}, {}, {})",
        quota_state.major, quota_state.ino, quota_state.minor
    );

    // open fd for filesystem root

    let fs_root = match OpenOptions::new().read(true).open(&FS_ROOT_PATH) {
        Ok(f) => f,
        Err(e) => panic!(
            "open: {}\nFailed to open filesystem root at {}",
            e, FS_ROOT_PATH
        ),
    };

    if VERBOSE {
        eprintln!("INFO\topened filesystem root: {FS_ROOT_PATH}");
    }

    // setup walk_inodes struct

    let first = scoutwrap::WalkInodesEntry {
        major: starting_state.major,
        ino: starting_state.ino,
        minor: starting_state.minor,
    };

    let last = scoutwrap::WalkInodesEntry {
        major: u64::MAX,
        ino: u64::MAX,
        minor: u32::MAX,
    };

    let mut walk_inodes_arg = scoutwrap::WalkInodes {
        first,
        last,
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
            if VERBOSE {
                eprintln!("last batch detected");
            }
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
            if entry.major == starting_state.major
                && entry.ino == starting_state.ino
                && entry.minor == starting_state.minor
            {
                if VERBOSE {
                    eprintln!("INFO\tskipping starting value {:?}", entry);
                }

                continue;
            }

            // stop if we are going to get ahead of quota_update
            if entry.major >= quota_state.major && entry.minor >= quota_state.minor {
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
                // print inodes and paths to stdout and all logs to stderr
                println!("{}\t\0{}", path, entry.ino);

                // set final state to the last file processed. This means the last file will be processed again in the next run, but this tool is idempotent.

                final_state = entry.clone();
            }
        }

        if VERBOSE {
            eprintln!("INFO\tfinished batch with current state {:?}", final_state);
        }

        // save state on last batch
        if last_batch {
            // update state file with final state
            if final_state.major != starting_state.major && final_state.major != 0 {
                let mut new_state_file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&STATE_SWAP_FILE)
                    .expect("failed to open state swp file");

                let write_str = format!(
                    "{}\n{}\n{}",
                    final_state.major, final_state.ino, final_state.minor
                );

                if let Err(e) = new_state_file.write_all(write_str.as_bytes()) {
                    panic!("failed to write new state: {}", e);
                }

                if let Err(e) = std::fs::rename(&STATE_SWAP_FILE, &STATE_FILE) {
                    panic!("failed to rename state swp file: {}", e)
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

fn read_state_from_file(path: &str) -> Result<WalkInodesEntry, String> {
    match OpenOptions::new().read(true).open(path) {
        Ok(f) => {
            let mut reader = BufReader::new(&f);
            let mut starting_state_str = String::new();

            if let Err(e) = reader.read_to_string(&mut starting_state_str) {
                panic!("read_to_string: {}", e);
            }

            let input_vec: Vec<String> = starting_state_str
                .split("\n")
                .map(|s| s.to_string())
                .collect();

            Ok(WalkInodesEntry {
                major: input_vec[0]
                    .trim()
                    .parse()
                    .expect("state file does not contain valid integer"),
                ino: input_vec[1]
                    .trim()
                    .parse()
                    .expect("state file does not contain valid integer"),
                minor: input_vec[2]
                    .trim()
                    .parse()
                    .expect("state file does not contain valid integer"),
            })
        }
        Err(e) => {
            if e.kind() == ErrorKind::NotFound {
                Err(String::from("file not found: {path}"))
            } else {
                panic!("open: {}\nFailed to open file", e);
            }
        }
    }
}

/// parent-finder
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Number of inodes to process in a single batch
    #[arg(short, long, default_value_t = 65536)]
    batch_size: usize,

    /// Print details for each processing step for each file. For debugging purposes
    #[arg(short, long)]
    verbose: bool,

    /// State file path (and state swap file)
    #[arg(short = 'p', long, default_value_t = String::from(".state"))]
    state_file_path: String,

    /// Root of ScoutFS filesystem
    #[arg(short, long)]
    root_scoutfs: String,

    /// Quota state file path
    #[arg(short, long)]
    quota_state_file_path: String,

    /// Check if the reindexer validated state before proceeding with future batches
    #[arg(long)]
    validate_reindex: bool,
}
