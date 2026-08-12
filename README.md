# gufi_change_finder

A tool that does incremental GUFI reindexing by only reindexing changed subdirectories of a GUFI tree instead of the entire tree by using ScoutFS changelogs.  

## Building

Rust and Cargo must be set up to build. Run: ```cargo build```

The build script assumes ScoutFS is an already compiled subdirectory of src: ```src/scoutfs```. It could also be a link to an existing build. 

The user is responsible for setting up a ScoutFSfilesystem and obtaining the ScoutFS source: https://github.com/versity/scoutfs.git. The scoutfs source does not need to be compiled to satisfy the dependency requirement for  gufi_change_finder.

The C ioctl wrapper scoutwrap is automatically compiled by the build script.

## Running

Must run as root.

It is highly recommended to create a runscript or systemd service to run this tool due to various path arguments.

A ScoutFS filesystem must be mounted. 

## Behavior

This tool has two components, the change finder and the reindexer. The change finder produces a list of subdirectories to reindex, and the reindexer generates new GUFI indexes and updates the GUFI tree. 

### gufi_change_finder

Only processes files changed since the last execution. Interfaces with the ScoutFS ioctl to obtain batches of recently changed files. Iterates through the batches and create a tree structure that maps the highest level changes in the filesystem. At the end of the run, outputs a file with the highest changed subdirectories. 

The tree structure is managed by a separate library in ```src/highest_change_tree.rs``` that can be used by other tools needing a similar workflow.

### reindexer.py

Processes output files from gufi_change_finder. Reindexes the subdirectories in a working directory, pivots them into the GUFI tree, and regenerates tree summaries.
