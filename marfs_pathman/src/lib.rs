/// Returns true if path is an internal component to the MarFS filesystem refrence tree, including
/// the quota file MDAL_datasize, anything in the reference tree MDAL_reference and the 
/// MDAL_subspaces directory itself. 
/// Assumes a valid path from the ScoutFS ioctl is provided.
pub fn gufi_filter(root_path: &str, path: &str) -> bool {

    // separate path by / into vector
    let mut path_vec: Vec<&str> = path.split('/').collect();
    path_vec.insert(0, root_path);

    // iterate over path_vec looking for marfs internal keywords in the correct structural
    // locations. The mdal-root is structured like /root/MDAL_*/subspaces/MDAL_*/subspaces.
    for i in 1 .. path_vec.len() {
        // odd indices 
        if i % 2 == 1 { 
            if path_vec[i] == "MDAL_datasize" || path_vec[i] == "MDAL_reference" || (path_vec[i] == "MDAL_subspaces" && i == path_vec.len() - 1) {
                // return true if 
                return true;
            }
        }
    }

    return false;

}

/// return: empty on error
pub fn internal_to_user(internal_path: &str) -> String {
    // validate input with is_internal()
    String::new()
}

/*
/// return:: empty on error
pub fn user_to_internal(user_path: &str) -> String {
    String::new()
}
*/
