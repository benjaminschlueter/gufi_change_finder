#!/usr/bin/python3

import sys
import os
import stat
import subprocess
import argparse
import threading
import shutil
from pathlib import Path

def rm_worker(path):
    print(f"Starting removal of {path}")
    result = subprocess.run(["rm", "-rf", path])
    print(f"Finished removal of {path}")

parser = argparse.ArgumentParser(description="GUFI subtree reindex tool")

parser.add_argument("-g", "--gufi-path", type=str, required=True, help="path to GUFI build dir")
parser.add_argument("-i", "--index", type=str, required=True, help="path to GUFI tree root")
parser.add_argument("-w", "--workdir", type=str, default="./reindex_work", help="location of program working directory")
parser.add_argument("-t", "--threads", type=str, default=32, help="thread count for GUFI processes")
parser.add_argument("--validate-state", type=str, default="", help="path of state file to write a validation for")

args = parser.parse_args()

THREAD_COUNT=args.threads
GUFI_PATH=args.gufi_path
GUFI_INDEX_DIR=args.index
WORK_REINDEX_DIR=f"{args.workdir}/reindex"
WORK_OLD_DIR=f"{args.workdir}/old"
VALIDATE_STATE=args.validate_state
VALIDATE_STATE_PATH=""
if VALIDATE_STATE:
    VALIDATE_STATE_PATH=args.validate_state

if VALIDATE_STATE:
    if Path(f"{VALIDATE_STATE_PATH}.reindex_validate.swp").is_file():
        print("detected reindex_validate swap file, removing")
        os.remove(f"{VALIDATE_STATE_PATH}.reindex_validate.swp")

# check for GUFI commands in path, if GUFI_PATH not specified (shutil) 

result = subprocess.run(["mkdir", "-p", WORK_REINDEX_DIR], check=True)

# if WORK_OLD_DIR or WORK_INDEX_DIR  has unremoved contents, remove them before beginning
result = subprocess.run(["rm", "-rf", f"{WORK_OLD_DIR}"], True)
result = subprocess.run(["mkdir", "-p", WORK_OLD_DIR], check=True)
result = subprocess.run(["rm", "-rf", f"{WORK_REINDEX_DIR}"], True)
result = subprocess.run(["mkdir", "-p", WORK_REINDEX_DIR], check=True)

os.environ["MARFS_SEC_ROOT"] = "/var/marfs/mdal-root/sec-root"
os.environ["MARFS_CONFIG_PATH"] = "/opt/storage/marfs/install/etc/marfs-config.xml"

paths = []
rm_threads = []


# add all paths from stdin to list before executing steps
for line in sys.stdin:
    line = line.rstrip('\n')

    # Gufi Change Finder outputs inode too: drop that part of the string    
    # path[0]: MarFS reference path
    # path[1]: FUSE path with MarFS internals removed
    paths.append((line.split("\t\0")[0], line.split("\t\0")[2]))


# Generate new GUFI index for each path
# Guranteed to received directories and namespaces with valid user mappings
for path in paths:
    print(f"Reindexing {path[0]}")

    parent_fuse_path_split = path[1].split('/')[:-1]
    parent_fuse_path = "/".join(parent_fuse_path_split)
    
    # call gufi_dir2index with MarFS plugin
    result = subprocess.run([f"{GUFI_PATH}/src/gufi_dir2index", "-x", "--threads", str(THREAD_COUNT),  "--plugin", f"GUFI_MARFS_PLUGIN:{GUFI_PATH}/contrib/plugins/libmarfs_plugin.so", path[0], f"{WORK_REINDEX_DIR}{parent_fuse_path}"], stdout=subprocess.DEVNULL, check=True) 


for path in paths:
    print(f"Pivoting {path[1]}")
    
    parent_fuse_path_split = path[1].split('/')[:-1]
    parent_fuse_path = "/".join(parent_fuse_path_split)
   
    result = subprocess.run(["mkdir", "-p", f"{WORK_OLD_DIR}{parent_fuse_path}"]) 
    result = subprocess.run(["mkdir", "-p", f"{GUFI_INDEX_DIR}{parent_fuse_path}"]) 
    
    # move GUFI tree subdir to working dir
    # allowed to fail when the index is being generated for the first time and is not in the GUFI tree yet
    # ADD HANDLER TO PRODUCE WARNING?
    result = subprocess.run(["mv", f"{GUFI_INDEX_DIR}{path[1]}", f"{WORK_OLD_DIR}{parent_fuse_path}"], stderr=subprocess.PIPE, universal_newlines=True) 

    # if this index is not part of the GUFI tree yet, print a warning. On other move errors, fail.
    if result.returncode == 1:
        if "No such file or directory" in str(result.stderr):
            print(f"Warning: {GUFI_INDEX_DIR}{path[1]} does not exist in the GUFI tree")
        else:
            print(str(result.stderr).rstrip("\n"))
            exit()

    # move new reindexed subdir to GUFI tree
    result = subprocess.run(["mv", f"{WORK_REINDEX_DIR}{path[1]}", f"{GUFI_INDEX_DIR}{parent_fuse_path}"], check=True)

    # spawn a new thread to remove old index: could this spawn too many?
    thread = threading.Thread(target=rm_worker, args=(f"{WORK_OLD_DIR}{path[1]}",))
    rm_threads.append(thread)
    thread.start()
            

# wait until all rm threads finish and remove all residual tree structure in old dir
for thread in rm_threads:
    thread.join()

print(f"Cleaning up working dir {WORK_OLD_DIR}")
result = subprocess.run(["rm", "-rf", f"{WORK_OLD_DIR}"], check=True)
result = subprocess.run(["mkdir", "-p", WORK_OLD_DIR], check=True)

print(f"Cleaning up working dir {WORK_REINDEX_DIR}")
result = subprocess.run(["rm", "-rf", f"{WORK_REINDEX_DIR}"], check=True)
result = subprocess.run(["mkdir", "-p", WORK_REINDEX_DIR], check=True)


print(f"Regenerating treesummaries")
result = subprocess.run([f"{GUFI_PATH}/src/gufi_treesummary_all", GUFI_INDEX_DIR], stdout=subprocess.DEVNULL)

if result.returncode == 1: 
    if not Path(f"{GUFI_INDEX_DIR}/db.db").is_file():
        print("Error: GUFI tree root contains no db.db")
        print("The reindexer requires an existing GUFI tree before performing incrementals")
    
    sys.exit(1)

# write validation file 
if VALIDATE_STATE:
    shutil.copy(VALIDATE_STATE_PATH, f"{VALIDATE_STATE_PATH}.reindex_validate.swp")
    os.replace(f"{VALIDATE_STATE_PATH}.reindex_validate.swp", f"{VALIDATE_STATE_PATH}.reindex_validate")
    print(f"wrote state validation file {VALIDATE_STATE_PATH}.reindex_validate")


