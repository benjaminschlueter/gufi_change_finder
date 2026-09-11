# gufi_change_finder

Allows for incremental GUFI reindexing by only reindexing changed subdirectories of a GUFI tree instead of the entire tree. Consists of three components, a changelog parser, a tree management tool that ScoutFS changelogs. These tools are designed to be part of a common workflow, but be interchanged with others to serve different use cases.   

This workflow is designed to support ScoutFS, but can be used with slight modifications to support other filesystems. 

## Building

Rust and Cargo must be set up to build. Run: ```cargo build```

The build script assumes ScoutFS is an already compiled subdirectory located at ```scoutwrap/src/scoutfs```, or a link to an existing build. 

The user is responsible for setting up a ScoutFS filesystem and obtaining the ScoutFS source: https://github.com/versity/scoutfs.git. The scoutfs source does not need to be compiled to satisfy the dependency requirement for scoutwrap.

The scoutwrap C ioctl wrapper is automatically compiled by the build script.

Also requires a GUFI build to target for reindexing. 

## Running

Must run parse_changelog as root because it calls the ScoutFS ioctl.

It is highly recommended to create a runscript or systemd service to that contains the desired piped workflow desired. 

A ScoutFS filesystem must be mounted. 

## Workflow Components 

This workflow has three compoents. The parse_changelog crate prints inodes and paths of files that have changed since the last run. The tree_manager crate reads inodes/paths and outputs leaves of a tree which are the highest directories that changed. The reindexer python script generates updated subtree indices and pivots them into the existing index. 

### parse_changelog

Iterates over the ScoutFS changelog starting at the state written by the last execution in the state file. Collects batches of inodes, calls the inode ot path ioctl, and prints the inode/path pair for each changelog entry until the most recent point in the changelog is reached. parse_changelog assumes the ScoutFS based quota management tool is running as well, and will not proceed past the quota tool in the changelog. 

To start from the beginning of the changelog, write 0\n0\n0 to the state file. 

To ignore the quota state, point the quota state file argument at a file with very large integers MAX\nMAX\nMAX. 

parse_changelog writes a temporary state file on completion, which is then renamed over the previous state file when the reindexer exits cleanly and this changelog batch has been reindexed successfully. 

### tree_manager

Takes in inode/path pairs, and adds them to a ChangeTree from the highest_change_tree crate. After all pairs have been read, the tree's leaves will be the highest directories with a change that need to be reindexed. If the filesystem root is encountered, tree_manager will output just the root and terminate 

tree_manager also performs path translation of internal MarFS filesystem paths to the FUSE paths in the user tree. It also filters out items that should not have gufi_dir2index called on them, like internal MarFS directories and files. 

Outputs lines containing the ScoutFS path, inode and the user tree FUSE path.  
### reindexer.py

Takes in (internal path, inode, user path) lines. Reads in all lines until the pipe closes before doing any reindexing. For all reindex targets, calls gufi_dir2index to generate an index in a working directory. Then, pivots out the old sub indicies in the GUFI tree to the working directory and moves the new ones into their place. Spins up a removal thread for each old index. After all reindexing, pivoting and purging is done, the tree summaries will be regenerated at the root, and the state will be confirmed.  

## Supporting Crates

### scoutwrap

Provides a Rust wrapper around the ScoutFS C ioctl that is called in parse_changelog. Uses bindgen to create Rust binding to the ScoutFS C codebase, which is managed by a build script build.rs. At the moment, only provides implementations for ioctl functions necessary for parse_changelog.

### highest_change_tree

Implementation of ChangeTree based on the indextree crate. When a node above existing ones is added, everything below the new node will be trimmed so the new node becomes a leaf. Nodes that would end up below the existing leaves are ignored.  

A recursive parse function adds the leaves to a vector and returns it to the caller. 

### marfs_pathman

Provides a simple interface for filtering out MarFS internal paths and translating from MarFS internal paths to user paths. 

## Known Issues

When runs are frequent, items can appear in the change log twice in a row, leading to extra work if they are reindexed twice in consecutive runs for the same change. 

Storage footprint: can potentially have two root indices existing at the same time, since gufi_change_finder using a working directory for its reindexing.  
