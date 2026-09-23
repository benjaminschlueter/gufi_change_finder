use std::sync::LazyLock;

use regex::Regex;

/// Returns true if path is an internal component to the MarFS filesystem refrence tree, including
/// the quota file MDAL_datasize, anything in the reference tree MDAL_reference and the
/// MDAL_subspaces directory itself.
pub fn is_internal(path: &str) -> bool {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        // matches internal MarFS paths that have no mappings to the user tree
        Regex::new(
            "^/var/marfs/mdal-root/sec-root/(MDAL_subspaces/[^/]+/)*MDAL_([^/]+|reference.*)$",
        )
        .unwrap()
    });

    RE.is_match(path)
}

/// return: empty on error
pub fn internal_to_user(user_root_path: &str, internal_path: &str) -> String {
    if internal_path.is_empty() {
        return user_root_path.to_string();
    }

    let mut path_vec: Vec<&str> = internal_path.split('/').collect();
    path_vec.insert(0, user_root_path);

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
