/// Returns true if path is an internal component to the MarFS filesystem refrence tree, including
/// the quota file MDAL_datasize, anything in the reference tree MDAL_reference and the 
/// MDAL_subspaces directory itself. 
/// Assumes a valid path from the ScoutFS ioctl is provided.
pub fn filter_internal(path: &str) -> bool {

    // separate path by / into vector
    let path_vec: Vec<&str> = path.split('/').collect();

    // iterate over path_vec looking for marfs internal keywords in the correct structural
    // locations. The mdal-root is structured like /root/MDAL_*/subspaces/MDAL_*/subspaces.
    for i in 0 .. path_vec.len() {
        // odd indices 
        if i % 2 == 0 { 
            if path_vec[i] == "MDAL_datasize" || path_vec[i] == "MDAL_reference" || (path_vec[i] == "MDAL_subspaces" && i == path_vec.len() - 1) {
                // return false if this is an internal path
                return false;
            }
        }
    }

    return true;

}

/// return: empty on error
pub fn internal_to_user(internal_path: &str) -> String {

    let fuse_path = "/marfs";
    let mut path_vec: Vec<&str> = internal_path.split('/').collect();
    path_vec.insert(0, fuse_path);

    // all internal paths filtered out: just remove MDAL_subspaces to translate to user path

    path_vec
        .into_iter()
        .filter(|s| *s != "MDAL_subspaces")
        .collect::<Vec<&str>>()
        .join("/")

}

/*
/// return:: empty on error
pub fn user_to_internal(user_path: &str) -> String {
    String::new()
}
*/
