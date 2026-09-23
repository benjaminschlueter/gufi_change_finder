use regex::Regex;

/// Returns true if path is an internal component to the MarFS filesystem refrence tree, including
/// the quota file MDAL_datasize, anything in the reference tree MDAL_reference and the
/// MDAL_subspaces directory itself.
pub fn is_internal(marfs_mdal_root: &str, path: &str) -> bool {
    // matches internal MarFS paths that have no mappings to the user tree
    let re = Regex::new(&format!(
        "^{}/(MDAL_subspaces/[^/]+/)*MDAL_([^/]+|reference.*)$", marfs_mdal_root
    ))
    .unwrap();

    re.is_match(path)
}

/// return: empty on error
pub fn internal_to_user(user_root_path: &str, internal_path: &str) -> String {
    if internal_path.is_empty() {
        return user_root_path.to_string();
    }

    let mut path_vec: Vec<&str> = internal_path.split('/').collect();
    path_vec.insert(0, user_root_path);
   
    // find the lowest namespace of this path
    let mut i: i64 = -1; 
    loop {
        
        if i as usize + 2 >= path_vec.len() {
            break;
        }

        if path_vec[i as usize + 2] == "MDAL_subspaces" {
            i = i + 2;
            continue;
        }
        else {
            break;
        }
    }

    // i is index of last MDAL_subspaces, or -1 if none were found 
    if i > -1 {
       return path_vec.join("/"); 
    } 
    else {
        // drop all MDAL_subspaces that are part of the MarFS namespae heirarchy including the lowest namespace but not including any paths the user created below that
        return path_vec
            .iter()
            .take(i as usize)
            .copied()
            .filter(|s| *s != "MDAL_subspaces")
            .collect::<Vec<&str>>()
            .join("/")
    }
}

/*
/// return:: empty on error
pub fn user_to_internal(user_path: &str) -> String {
    String::new()
}
*/
