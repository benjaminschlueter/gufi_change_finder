use gufi_change_finder::*;

use std::fs::File;
use std::os::fd::AsRawFd;
use std::ffi::CStr;
use std::os::fd::BorrowedFd;
use std::slice;

pub const STR_BUF_SIZE: usize = 512;


/* Represents a point in the ScoutFS changelog.
 * @param major: major timestamp
 * @param ino: inode number
 * @param minor: minor timestamp
 */
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ScoutwrapWalkInodesEntry {
    pub major: u64,
    pub ino: u64,
    pub minor: u32,
}

/* To be used with scoutwrap_walk_inodes()
 * @param first: starting point in the change log
 * @param last: stop point in the change log
 * @param entries_vec: to be populated with ScoutwrapWalkInodesEntry structs. Will be overwritten at the end of the function. 
 * @param nr_entries: tells ScoutFS the limit of entry structs to fill the buffer with
 * @param index: see ScoutFS ioctl.h for which macro to set this with
 */
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ScoutwrapWalkInodes {
    pub first: ScoutwrapWalkInodesEntry,
    pub last: ScoutwrapWalkInodesEntry,
    pub entries_vec: Vec<ScoutwrapWalkInodesEntry>, // MUST BE EMPTY
    pub nr_entries: usize,
    pub index: u8,
}


/* WALK_INODES
 * Allocates and populates the entries_vec in user_arg to contain nr_entries entry structs from inodes within the minor:major range. This function allocates the buffer. The caller does not have to worry about setting up a buffer.  
 * Moves the callers struct inside, modifies and returns it. 
 */
#[allow(dead_code)]
pub fn scoutwrap_walk_inodes(root_fs: &File, mut user_arg: ScoutwrapWalkInodes) -> Result<ScoutwrapWalkInodes, String> {

    // create scoutfs_ioctl_walk_inodes and entries structs

    let first_c = scoutfs_ioctl_walk_inodes_entry {
        major: user_arg.first.major as __u64,
        ino: user_arg.first.ino as __u64,
        minor: user_arg.first.minor as __u32,
        _pad: [0u8; 4usize],
    };
    
    let last_c = scoutfs_ioctl_walk_inodes_entry {
        major: user_arg.last.major as __u64,
        ino: user_arg.last.ino as __u64,
        minor: user_arg.last.minor as __u32,
        _pad: [0u8; 4usize],
    };
    
    let entries_ptr;
    unsafe {
        entries_ptr = libc::calloc(user_arg.nr_entries as usize, std::mem::size_of::<scoutfs_ioctl_walk_inodes_entry>());
        if entries_ptr.is_null() {
            return Err(String::from("calloc: ") + &std::io::Error::last_os_error().to_string());
        }
    }


    let mut user = scoutfs_ioctl_walk_inodes {
        first: first_c,
        last: last_c,
        entries_ptr: entries_ptr as u64,
        nr_entries: user_arg.nr_entries as u32, 
        index: user_arg.index,
        _pad: [0u8; 11usize],
    };
   
    let entries_c; 
    unsafe {
        if wrap_walk_inodes(root_fs.as_raw_fd(), &mut user) == -1 {
            return Err(std::io::Error::last_os_error().to_string());
        }

        // takes ownership of the calloc buffer and will drop it
        entries_c = Vec::from_raw_parts(user.entries_ptr as *mut scoutfs_ioctl_walk_inodes_entry, user.nr_entries as usize, user.nr_entries as usize);
    }        
     
    let mut entries = Vec::<ScoutwrapWalkInodesEntry>::new();
    
    // add entries to the rust buffer until the end is found
    for entry in &entries_c {

        if entry.major == 0 && entry.ino == 0 && entry.minor == 0 {
            break;
        }

        let tmp = ScoutwrapWalkInodesEntry { 
            major: entry.major as u64, 
            ino: entry.ino as u64,
            minor: entry.minor as u32,
        };

        entries.push(tmp);
    }   

    user_arg.entries_vec = entries;

    Ok(user_arg)

}

/* Input for INO_PATH ioctl function
 */
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ScoutwrapInoPath {
    pub ino: u64,
    pub dir_ino: u64,
    pub dir_pos: u64,
    pub result_ptr: u64,
    pub result_bytes: usize,
}

/* Result output for INO_PATH ioctl function
 */
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ScoutwrapInoPathResult {
    pub ino: u64,
    pub dir_pos: u64,
    pub path_bytes: u16,
    pub path: String,
}

/* Return result of INO_PATH ioctl function, containing the path of the file with specified inode.
 * @param root_fs
 * @param path_arg: struct with input for scoutfs ioctl
 * @return ioctl result struct
 */
#[allow(dead_code)]
pub fn scoutwrap_ino_path(root_fs: &File, path_arg: ScoutwrapInoPath) -> Result<ScoutwrapInoPathResult, String> {
   
    let result_ptr;
    unsafe {
        result_ptr = libc::calloc(1, STR_BUF_SIZE);
    }

    let mut path_c = scoutfs_ioctl_ino_path {
        ino: path_arg.ino,
        dir_ino: path_arg.dir_ino,
        dir_pos: path_arg.dir_pos,
        result_ptr: result_ptr as u64,
        result_bytes: STR_BUF_SIZE as u16,
        _pad:[0u8; 6usize],
    };
    
    unsafe {
        if wrap_ino_path(root_fs.as_raw_fd(), &mut path_c) == -1 {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }

    let ret_str;
    unsafe {
        // ensure we only get the first string if buffer contains multiple
        let entries_c = Vec::from_raw_parts(path_c.result_ptr as *mut scoutfs_ioctl_ino_path_result, 1, 1);
        let path_ptr = entries_c[0].path.as_ptr() as *const i8;
        
        ret_str = CStr::from_ptr(path_ptr).to_str().unwrap().to_owned();
    }

    let ret_struct = ScoutwrapInoPathResult {
        ino: path_arg.ino,
        dir_pos: path_arg.dir_pos,
        path_bytes: path_arg.result_bytes as u16,
        path: ret_str,
    };

    return Ok(ret_struct)
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ScoutwrapListxattrHidden {
    pub id_pos: u64,
    pub xattr_list: Vec<String>, // possible to have more than 1 xattr
    pub buf_bytes: usize,
    pub hash_pos: u32,
}

/* Return a vector of owned string with the names of ScoutFS xattrs attached to a file.
 * @param fd: file descriptor for file being queried
 * @param xattr_arg
 * @return vector of owned strings with all xattr names
 */
#[allow(dead_code)]
pub fn scoutwrap_listxattr_hidden(fd: BorrowedFd, xattr_arg: ScoutwrapListxattrHidden) -> Result<Vec<String>, String> {
    
    unsafe {

        let buf = libc::calloc(1, STR_BUF_SIZE);

        let mut existing_xattrs = scoutfs_ioctl_listxattr_hidden {
                id_pos: xattr_arg.id_pos,
                buf_ptr: buf as u64,
                buf_bytes: STR_BUF_SIZE as u32,
                hash_pos: xattr_arg.hash_pos,
        };

        if wrap_listxattr_hidden(fd.as_raw_fd(), &mut existing_xattrs) == -1 {
            return Err(std::io::Error::last_os_error().to_string());
        }

        // create vector of strings to return
        let buf = slice::from_raw_parts(existing_xattrs.buf_ptr as *const u8, existing_xattrs.buf_bytes as usize);
        let xattr_str_vec = buf.split(|b| *b == 0) // create an iterator over null terminated string subslices
                .filter_map(|slice| {

                        if slice.is_empty() {
                            return None;
                        }

                        match std::str::from_utf8(slice) {
                            Ok(s) => Some(s.to_owned()),
                            Err(e) => Some(String::from("error: failed to parse slice into utf8")),

                            // CALLER IS RESPONSIBLE FOR CHECKING VECTOR FOR ERROR STRINGS
                        }
                }).collect();

        return Ok(xattr_str_vec);
    }
}









