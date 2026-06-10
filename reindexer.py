#!/usr/bin/python3.11

import sys
import os
import subprocess
import argparse

parser = argparse.ArgumentParser(description="GUFI subtree reindex tool")

parser.add_argument("-i", "--index", type=str, required=True, help="path to GUFI tree root")
parser.add_argument("-w", "--workdir", type=str, default="./reindex_work", help="location of program working directory")
parser.add_argument("-t", "--threads", type=str, default=32, help="thread count for GUFI processes")

args = parser.parse_args()

THREAD_COUNT=args.threads

GUFI_INDEX_DIR=args.index
GCF_OUTPUT_DIR="/home/bschlueter/gufi_change_finder/output"
WORK_REINDEX_DIR=f"{args.workdir}/reindex"
WORK_OLD_DIR=f"{args.workdir}/old"

tree_root_paths = ["/marfs/mdal-root/parent-testing"]

output_file_list = os.listdir(GCF_OUTPUT_DIR)

if len(output_file_list) == 0:
    print("output directory is empty... exiting:")
    exit()

result = subprocess.run(["mkdir", "-p", WORK_REINDEX_DIR], check=True)
result = subprocess.run(["mkdir", "-p", WORK_OLD_DIR], check=True)

for file in output_file_list:
    print(f"processing {file}")

    with open(f"{GCF_OUTPUT_DIR}/{file}") as f:
        lines = f.readlines()        

    # Gufi Change Finder outputs inode too: drop that part of the string    
    for i in range(len(lines)):
        lines[i] = lines[i].split('\x00')[0]

    # generate new GUFI index for each path
    for path in lines:
        parent_path_list = path.split('/')[:-1]
        parent_path = "/".join(parent_path_list)
        
        print(f"reindexing {path}")

        result = subprocess.run(["mkdir", "-p", f"{WORK_REINDEX_DIR}{parent_path}"], check=True)
        
        # won't generate anything for files
        result = subprocess.run(["gufi_dir2index", "--thread", str(THREAD_COUNT), path, f"{WORK_REINDEX_DIR}{parent_path}"], check=True, capture_output=True)

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
    result = subprocess.run(["gufi_treesummary_all", GUFI_INDEX_DIR], capture_output=True)

    print("cleaning up working directory")
    result = subprocess.run(["rm", "-rf", WORK_OLD_DIR])
    result = subprocess.run(["mkdir", "-p", WORK_OLD_DIR], check=True)
    
    os.remove(f"{GCF_OUTPUT_DIR}/{file}")

# move treesummary generation here?
