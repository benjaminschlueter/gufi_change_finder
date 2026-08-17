/* The ScoutFS ioctl is kernel code and does not provide a library that Rust can link to.
 * This file serves as a wrapper for the ioctl and any ScoutFS code that needs to be called
 * from C.
 */

#include <sys/ioctl.h>
#include "utils/src/util.h"
#include "kmod/src/ioctl.h"

// WALK_INODES
int wrap_walk_inodes(int root_fd, struct scoutfs_ioctl_walk_inodes *user);

// INO_PATH
int wrap_ino_path(int root_fd, struct scoutfs_ioctl_ino_path *path);

// LISTXATTR_HIDDEN
int wrap_listxattr_hidden(int root_fd, struct scoutfs_ioctl_listxattr_hidden *existing_attrs);

