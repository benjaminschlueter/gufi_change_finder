// SPDX-License-Identifier: MIT
// Copyright 2025. Triad National Security, LLC.

//! Description:
//! This library provides an interface to create and manage a tree with leaves that represent the
//! highest directories in which changes occured in a filesystem. It is intended to receive path
//! input from a changelog.
//!
//! Create a tree by calling highest_change_tree_create() and highest_change_tree_new_node with the
//! filesystem root path and inode number. A variable in the calling program should own the Arena
//! with the tree data.
//!
//! As paths are provided to highest_change_tree_add_path, the tree will be expanded so new
//! nodes that would end up below the leaves are ignored, and new nodes added above the leaves will trim
//! nodes in the tree below them and become new leaves. To reduce the trees memory footprint, nodes
//! with large numbers of children will be trimmed when their child count reaches the defined limit.
//!
//! When the tree is fully generated, it can be parsed into a list with highest_change_tree_parse_leaves.
//! This function traverses the tree recursively and adds leaves to a vector that will be returned.

use indextree::{Arena, NodeId};
use std::fs;

const MAX_CHILD_COUNT: usize = 1024; // if a node has more than this many children, give up adding more and rescan the whole parent dir

#[derive(Debug, Clone, PartialEq)]
pub struct OutputListData {
    pub internal_path: String,
    pub fuse_path: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreeData {
    pub path: String,
}

#[derive(Debug)]
pub struct ChangeTree {
    arena: Arena<TreeData>,
    root: NodeId,
}

impl ChangeTree {
    /// Create a new ChangeTree. Allocates Arena and sets root node.
    pub fn new(root: TreeData) -> Self {
        let mut arena = Arena::new();
        let root = arena.new_node(root);

        Self { arena, root }
    }

    /// The full process of adding a node with a path to the tree and trimming when necessary. Paths should be relative to the root node. Expects an empty string to represent the filesystem root.
    pub fn add_path(tree: &mut ChangeTree, data: TreeData) {
        let path_vec: Vec<&str> = data.path.split('/').collect();

        let mut cur_node = tree.root;
        let mut child;
        let path_vec_last = path_vec[path_vec.len() - 1];

        // check for root first
        if data.path.is_empty() {
            Self::trim_below(tree, tree.root);
            return;
        }

        for entry in &path_vec {
            // check if cur_node has child named entry
            if let Some(c) = cur_node
                .children(&tree.arena)
                .find(|&child| *tree.arena[child].get().path == *entry.to_owned())
            {
                // if leaf, break because this is already being rescanned
                if c.children(&tree.arena).count() == 0 {
                    break;
                }

                child = c;
            }
            // node not found: add it
            else {
                child = tree.arena.new_node(TreeData {
                    path: entry.to_string(),
                });

                // add new child to cur_node
                cur_node.append(child, &mut tree.arena);

                // check if cur_node has too many children
                let child_count = cur_node.children(&tree.arena).count();

                if child_count > MAX_CHILD_COUNT {
                    // trim if node has too many children and just rescan that node

                    // remove all children and grand children of cur_node

                    Self::trim_below(tree, cur_node);

                    // cur_node becomes a leaf and no more children will be added

                    break;
                }
            }

            // if child is at bottom, trim below if node has children
            if *entry == path_vec_last && child.children(&tree.arena).count() > 0 {
                Self::trim_below(tree, child);
            }

            // update cur_node for next iteration
            cur_node = child;
        }
    }

    /// Trims all nodes below node with NodeId. A helper for add_path.
    fn trim_below(tree: &mut ChangeTree, node: NodeId) {
        let children: Vec<NodeId> = node.children(&tree.arena).collect();

        for c in children {
            c.remove_subtree(&mut tree.arena);
        }
    }

    /// This recursive function traverses the tree to the leaves and adds them to a vector for output.
    pub fn parse_leaves(&self) -> Vec<OutputListData> {
        let mut parent_list = Vec::new();

        // check if tree is only root
        if self.root.children(&self.arena).count() == 0 {
            parent_list.push(OutputListData {
                internal_path: self.arena[self.root].get().path.clone(),
                fuse_path: String::new(),
            });
        } else {
            self.parse_leaves_inner(
                self.root,
                &mut parent_list,
                self.arena[self.root].get().path.clone(),
            );
        }

        parent_list
    }

    fn parse_leaves_inner(
        &self,
        node: NodeId,
        parent_list: &mut Vec<OutputListData>,
        partial_path: String,
    ) {
        // process all children before descending
        for child in node.children(&self.arena) {
            let mut partial_path_new = format!("{}/{}", partial_path, self.arena[child].get().path);

            if self.arena[child].first_child().is_none() {
                let is_file = fs::metadata(&partial_path_new)
                    .unwrap_or_else(|_| panic!("failed to stat {}", partial_path_new))
                    .is_file();

                if is_file {
                    // add parent instead
                    partial_path_new = partial_path.clone();
                }

                // generate FUSE path from tree reference path
                let fuse_path = String::new();

                // add leaf directories and parents of files
                parent_list.push(OutputListData {
                    internal_path: partial_path_new.clone(), // output list contains absolute path
                    fuse_path,
                });

                // if a file is found and the parent is added, no need to keep processing children
                if is_file {
                    return;
                }
            }
        }

        // descend to next level after leaves are processed
        for child in node.children(&self.arena) {
            let partial_path_new = format!("{}/{}", partial_path, self.arena[child].get().path);

            // node: extend path and keep recursing
            Self::parse_leaves_inner(self, child, parent_list, partial_path_new);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highest_change_tree_all() {
        let fs_root = format!("{}/tests", env!("CARGO_MANIFEST_DIR"));

        create_test_path(&fs_root);

        let mut tree = ChangeTree::new(TreeData {
            path: fs_root.clone(),
        });

        // test with only root in tree
        assert_eq!(
            tree.parse_leaves(),
            vec![OutputListData {
                internal_path: format!("{}", fs_root),
                fuse_path: String::new()
            }]
        );

        // make sure adding root in an only root tree works; root takes form of empty string
        ChangeTree::add_path(
            &mut tree,
            TreeData {
                path: String::from(""),
            },
        );
        assert_eq!(
            tree.parse_leaves(),
            vec![OutputListData {
                internal_path: format!("{}", fs_root),
                fuse_path: String::new()
            }]
        );

        let path = "a";
        ChangeTree::add_path(
            &mut tree,
            TreeData {
                path: String::from(path),
            },
        );
        create_test_path(&format!("{}/{}", &fs_root, path));
        assert_eq!(
            tree.parse_leaves(),
            vec![OutputListData {
                internal_path: format!("{}/a", fs_root),
                fuse_path: String::new()
            }]
        );

        // add a path below a: a remains a leaf
        let path = "a/b";
        ChangeTree::add_path(
            &mut tree,
            TreeData {
                path: String::from(path),
            },
        );
        create_test_path(&format!("{}/{}", &fs_root, path));
        assert_eq!(
            tree.parse_leaves(),
            vec![OutputListData {
                internal_path: format!("{}/a", fs_root),
                fuse_path: String::new()
            }]
        );

        // add new branch from root 2 deep: output should include c/d
        let path = "c/d";
        ChangeTree::add_path(
            &mut tree,
            TreeData {
                path: String::from(path),
            },
        );
        create_test_path(&format!("{}/{}", &fs_root, path));
        assert_eq!(
            tree.parse_leaves(),
            vec![
                OutputListData {
                    internal_path: format!("{}/a", fs_root),
                    fuse_path: String::new()
                },
                OutputListData {
                    internal_path: format!("{}/c/d", fs_root),
                    fuse_path: String::new()
                }
            ]
        );

        // add new branch in c: output should include the new branch
        let path = "c/e";
        ChangeTree::add_path(
            &mut tree,
            TreeData {
                path: String::from(path),
            },
        );
        create_test_path(&format!("{}/{}", &fs_root, path));
        assert_eq!(
            tree.parse_leaves(),
            vec![
                OutputListData {
                    internal_path: format!("{}/a", fs_root),
                    fuse_path: String::new()
                },
                OutputListData {
                    internal_path: format!("{}/c/d", fs_root),
                    fuse_path: String::new()
                },
                OutputListData {
                    internal_path: format!("{}/c/e", fs_root),
                    fuse_path: String::new()
                }
            ]
        );

        // add same path again: output should stay the same
        let path = "c/e";
        ChangeTree::add_path(
            &mut tree,
            TreeData {
                path: String::from(path),
            },
        );
        create_test_path(&format!("{}/{}", &fs_root, path));
        assert_eq!(
            tree.parse_leaves(),
            vec![
                OutputListData {
                    internal_path: format!("{}/a", fs_root),
                    fuse_path: String::new()
                },
                OutputListData {
                    internal_path: format!("{}/c/d", fs_root),
                    fuse_path: String::new()
                },
                OutputListData {
                    internal_path: format!("{}/c/e", fs_root),
                    fuse_path: String::new()
                }
            ]
        );

        // add path above the leaves: this node should become a new leaf
        let path = "c";
        ChangeTree::add_path(
            &mut tree,
            TreeData {
                path: String::from(path),
            },
        );
        create_test_path(&format!("{}/{}", &fs_root, path));
        assert_eq!(
            tree.parse_leaves(),
            vec![
                OutputListData {
                    internal_path: format!("{}/a", fs_root),
                    fuse_path: String::new()
                },
                OutputListData {
                    internal_path: format!("{}/c", fs_root),
                    fuse_path: String::new()
                }
            ]
        );

        // add root to trim all below root
        ChangeTree::add_path(
            &mut tree,
            TreeData {
                path: String::from(""),
            },
        );
        assert_eq!(
            tree.parse_leaves(),
            vec![OutputListData {
                internal_path: format!("{}", fs_root),
                fuse_path: String::new()
            }]
        );
    }

    fn create_test_path(path: &str) {
        if let Err(e) = fs::create_dir_all(path) {
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                panic!("failed to create test dir {}: {}", path, e);
            }
        }
    }
}
