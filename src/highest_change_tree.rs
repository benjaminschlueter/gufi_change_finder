/* Description
 *
 */

use indextree::{Arena, NodeId};

const MAX_CHILD_COUNT: usize = 1024; // if a node has more than this many children, give up adding more and rescan the whole parent dir

#[derive(Debug)]
pub struct TreeData {
    pub name: String,
    pub ino: u64,
}

pub fn highest_change_tree_create() -> Arena<TreeData> {
    Arena::new()
}

pub fn highest_change_tree_new_node(arena: &mut Arena<TreeData>, root_data: TreeData) -> NodeId {
   arena.new_node(root_data) 
}

pub fn highest_change_tree_add_path(arena: &mut Arena<TreeData>, tree_root: NodeId, path: String, ino: u64) {

        let mut path_vec: Vec<&str> = path.split('/').collect();
        let tree_root_name = arena[tree_root].get().name.clone();
        path_vec.insert(0, &tree_root_name); // path_vec must be length 2 or greater

        let mut cur_node = tree_root;
        let mut child;
        let path_vec_last = path_vec[path_vec.len() - 1];

        for entry in &path_vec[1..] {
            // skip tree_root path_vec entry

            // check if cur_node has child named entry
            if let Some(c) = cur_node
                .children(arena)
                .find(|&child| *arena[child].get().name == *entry.to_owned())
            {
                // if leaf, break because this is already being rescanned
                if c.children(arena).count() == 0 {
                    break;
                }

                child = c;
            }
            // node not found: add it
            else {
                // if adding a leaf, set the inode in TreeData
                if *entry == path_vec_last {
                    child = arena.new_node(TreeData {
                        name: entry.to_string(),
                        ino: ino,
                    });
                } else {
                    child = arena.new_node(TreeData {
                        name: entry.to_string(),
                        ino: 0,
                    });
                }
                
                // add new child to cur_node
                cur_node.append(child, arena);

                // check if cur_node has too many children
                let child_count = cur_node.children(arena).count();

                if child_count > MAX_CHILD_COUNT {
                    // trim if node has too many children and just rescan that node

                    // remove all children and grand children of cur_node

                    highest_change_tree_trim_below(arena, cur_node);

                    // cur_node becomes a leaf and no more children will be added

                    break;
                }
            }

            // if child is at bottom, trim below if node has children
            if *entry == path_vec_last && child.children(arena).count() > 0 {

                highest_change_tree_trim_below(arena, child);
            }

            // update cur_node for next iteration
            cur_node = child;
    }
}


pub fn highest_change_tree_trim_below(arena: &mut Arena<TreeData>, node: NodeId) {
        let children: Vec<NodeId> = node.children(arena).collect();

        for c in children {
            c.remove_subtree(arena);
        }
}

pub fn highest_change_tree_parse_leaves(
    node: NodeId,
    parent_list: &mut Vec<TreeData>,
    partial_path: String,
    arena: &Arena<TreeData>,
) {
    for child in node.children(arena) {
        let partial_path_new = format!("{}/{}", partial_path, arena[child].get().name);

        if arena[child].first_child().is_none() {
            // leaf: add new TreeData with abs path instead of relative and inode
            
            // filter 
            
            parent_list.push(TreeData {
                name: partial_path_new,
                ino: arena[child].get().ino,
            });

            continue;
        }

        // node: extend path and keep recursing
        highest_change_tree_parse_leaves(child, parent_list, partial_path_new, arena);
    }
}
