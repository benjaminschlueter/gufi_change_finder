# quota_update

A tool for updating MarFS namespace quotas using ScoutFS xattr capabilities.

## Building

Cargo must be set up to build. Run: ```cargo build```

The build script assumes MarFS and ScoutFS are already compiled subdirectories of src: ```src/marfs``` and ```src/scoutfs```. These could also be links to existing builds. 

The user is responsible for manually compiling MarFS and ScoutFS

The wrappers nswrap and scoutwrap are automatically compiled by the build script

## Running

Must run as root.

It is highly recommended to create a runscript or systemd service to run this tool due to various path arguments.

## Behavior

This tool has two components, the change finder and the reindexer. The change finder produces a list of subdirectories to reindex, and the reindexer generates new GUFI indexes and updates the GUFI tree. 

### gufi_change_finder

Only processes files changed since the last execution. Interfaces with the ScoutFS ioctl to obtain batches of recently changed files. Iterates through the batches and create a tree structure that maps the highest level changes in the filesystem. At the end of the run, outputs a file with the highest changed subdirectories. 

### reindexer.py

Processes output files from gufi_change_finder. Reindexes the subdirectories in a working directory, pivots them into the GUFI tree, and regenerates tree summaries.
