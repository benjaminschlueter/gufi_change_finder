#!/usr/bin/python3

import sys
import os
import stat
import subprocess
import argparse
from pathlib import Path

parser = argparse.ArgumentParser(description="GUFI subtree reindex tool")

parser.add_argument("-i", "--index", type=str, required=True, help="path to GUFI tree root")
parser.add_argument("-w", "--workdir", type=str, default="./reindex_work", help="location of program working directory")
parser.add_argument("-t", "--threads", type=str, default=32, help="thread count for GUFI processes")
parser.add_argument("-o", "--output-file-dir", type=str, default="./output", help="location of files output by main executable")

args = parser.parse_args()

THREAD_COUNT=args.threads

GUFI_PATH="/opt/storage/tmp/GUFI/build"
GUFI_INDEX_DIR=args.index
GCF_OUTPUT_DIR=args.output_file_dir
WORK_REINDEX_DIR=f"{args.workdir}/reindex"
WORK_OLD_DIR=f"{args.workdir}/old"

tree_root_paths = ["/marfs/mdal-root/parent-testing"]

output_file_list = os.listdir(GCF_OUTPUT_DIR)

if len(output_file_list) == 0:
    print("output directory is empty... exiting:")
    exit()

# check for GUFI commands in path, if GUFI_PATH not specified (shutil) 

result = subprocess.run(["mkdir", "-p", WORK_REINDEX_DIR], check=True)
result = subprocess.run(["mkdir", "-p", WORK_OLD_DIR], check=True)

os.environ["MARFS_SEC_ROOT"] = "/var/marfs/mdal-root/sec-root"
os.environ["MARFS_CONFIG_PATH"] = "/opt/storage/marfs/install/etc/marfs-config.xml"

# iterate over all files in output directory
for file in output_file_list:
    print(f"processing {file}")

    with open(f"{GCF_OUTPUT_DIR}/{file}") as f:
        lines = f.readlines()        

    paths = []

    # Gufi Change Finder outputs inode too: drop that part of the string    
    for line in lines:
        paths.append((line.split('\x00')[0], line.split('\x00')[2]))

    # Generate new GUFI index for each path
    # Guranteed to received directories and namespaces with valid user mappings
    for path in paths:
        print(f"reindexing {path[0]}")

        parent_fuse_path_split = path[1].split('/')[:-1]
        parent_fuse_path = "/".join(parent_fuse_path_split)
        
        print(f"parent_fuse_path: {parent_fuse_path}")

        #result = subprocess.run(["mkdir", "-p", f"{WORK_REINDEX_DIR}{parent_fuse_path}"], check=True)
        
        # call gufi_dir2index with MarFS plugin
        result = subprocess.run([f"{GUFI_PATH}/src/gufi_dir2index", "-x", "--threads", str(THREAD_COUNT),  "--plugin", f"GUFI_MARFS_PLUGIN:{GUFI_PATH}/contrib/plugins/libmarfs_plugin.so", path[0], f"{WORK_REINDEX_DIR}{parent_fuse_path}"], cwd=GUFI_PATH, check=True) 

    exit()

    for path in lines:
        parent_path_list = path.split('/')[:-1]
        parent_path = "/".join(parent_path_list)

        print(f"pivoting {path}")
       
        result = subprocess.run(["mkdir", "-p", f"{WORK_OLD_DIR}{parent_path}"], check=True) 

        # print(result.stdout)

        result = subprocess.run(["mkdir", "-p", f"{GUFI_INDEX_DIR}{parent_path}"], check=True) 
        
        # both of these will fail for files to not be pivoted and succeed for dirs
        result = subprocess.run(["mv", f"{GUFI_INDEX_DIR}{path}", f"{WORK_OLD_DIR}{parent_path}"]) # allowed to fail when the index is being generated for the first time and is not in the GUFI tree yet
        result = subprocess.run(["mv", f"{WORK_REINDEX_DIR}{path}", f"{GUFI_INDEX_DIR}{path}"], check=True)
    
    print(f"regenerating treesummaries after processing {file}")
    result = subprocess.run([f"{GUFI_PATH}/src/gufi_treesummary_all", GUFI_INDEX_DIR], capture_output=True)

    print("cleaning up working directory")
    result = subprocess.run(["rm", "-rf", WORK_OLD_DIR])
    result = subprocess.run(["mkdir", "-p", WORK_OLD_DIR], check=True)
    
    os.remove(f"{GCF_OUTPUT_DIR}/{file}")

# move treesummary generation here?
