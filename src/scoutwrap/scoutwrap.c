/* The ScoutFS ioctl is kernel code and does not provide a library that Rust can link to.
 * This file serves as a wrapper for the ioctl and any ScoutFS code that needs to be called
 * from C.
 */

#include <stdio.h>

#include "scoutwrap.h"

// WALK_INODES
int wrap_walk_inodes(int root_fd, struct scoutfs_ioctl_walk_inodes *user) {
	return ioctl(root_fd, SCOUTFS_IOC_WALK_INODES, user); // access errno from Rust
}

// INO_PATH
int wrap_ino_path(int root_fd, struct scoutfs_ioctl_ino_path *path) {
	return ioctl(root_fd, SCOUTFS_IOC_INO_PATH, path); // access errno from Rust
}

// LISTXATTR_HIDDEN
int wrap_listxattr_hidden(int root_fd, struct scoutfs_ioctl_listxattr_hidden *existing_attrs) {
	return ioctl(root_fd, SCOUTFS_IOC_LISTXATTR_HIDDEN, existing_attrs); // access errno from Rust
}
