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

const MAX_CHILD_COUNT: usize = 1024; // if a node has more than this many children, give up adding more and rescan the whole parent dir

#[derive(Debug, Clone)]
pub struct TreeData {
    pub name: String,
    pub ino: u64,
}

#[derive(Debug)]
pub struct ChangeTree {
    arena: Arena<TreeData>,
    pub root: NodeId,
}

impl ChangeTree {

    /// Create a new ChangeTree. Allocates Arena and sets root node.
    pub fn new(root: TreeData) -> Self {
        let mut arena = Arena::new();
        let root = arena.new_node(root);

        Self { arena, root }
    }

    ///The full process of adding a node with a path to the tree and trimming when necessary.
    pub fn add_path(
        tree: &mut ChangeTree,
        path: String,
        ino: u64,
    ) {
        let mut path_vec: Vec<&str> = path.split('/').collect();
        let tree_root_name = tree.arena[tree.root].get().name.clone();
        path_vec.insert(0, &tree_root_name); // path_vec must be length 2 or greater

        let mut cur_node = tree.root;
        let mut child;
        let path_vec_last = path_vec[path_vec.len() - 1];

        for entry in &path_vec[1..] {
            // skip tree_root path_vec entry

            // check if cur_node has child named entry
            if let Some(c) = cur_node
                .children(&tree.arena)
                .find(|&child| *tree.arena[child].get().name == *entry.to_owned())
            {
                // if leaf, break because this is already being rescanned
                if c.children(&tree.arena).count() == 0 {
                    break;
                }

                child = c;
            }
            // node not found: add it
            else {
                // if adding a leaf, set the inode in TreeData
                if *entry == path_vec_last {
                    child = tree.arena.new_node(
                        TreeData {
                            name: entry.to_string(),
                            ino: ino,
                        }
                    );
                } else {
                    child = tree.arena.new_node(
                        TreeData {
                            name: entry.to_string(),
                            ino: 0,
                        }
                    );
                }

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

    /// Trims all nodes below node with NodeId. This function is public so the caller can trim below the
    /// root in the case where it is found.
    pub fn trim_below(tree: &mut ChangeTree, node: NodeId) {
        let children: Vec<NodeId> = node.children(&tree.arena).collect();

        for c in children {
            c.remove_subtree(&mut tree.arena);
        }
    }

    /// This recursive function traverses the tree to the leaves and adds them to a vector for output.
    pub fn parse_leaves(
        &self,
        parent_list: &mut Vec<TreeData>,
        partial_path: String,
    ) {
        self.parse_leaves_inner(self.root, parent_list, partial_path);
    }   


    fn parse_leaves_inner(
        &self,
        node: NodeId,
        parent_list: &mut Vec<TreeData>,
        partial_path: String,
    ) {
     
        for child in node.children(&self.arena) {
            let partial_path_new = format!("{}/{}", partial_path, self.arena[child].get().name);

            if self.arena[child].first_child().is_none() {
                // leaf: add new TreeData with abs path instead of relative and inode

                // stat to determine if file and parent needs to be added instead (could do some tree
                // child scan optimization here: if other files belong to this parent, skip those
                // files)

                // generate FUSE path from tree reference path and add that to list as well 

                parent_list.push(TreeData {
                    name: partial_path_new,
                    ino: self.arena[child].get().ino,
                });

                continue;
            }

            // node: extend path and keep recursing
            Self::parse_leaves_inner(self, child, parent_list, partial_path_new);
        }
    }

}
