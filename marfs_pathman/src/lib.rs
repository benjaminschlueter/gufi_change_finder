use std::sync::OnceLock;

use regex::Regex;

static RE: OnceLock<Regex> = OnceLock::new();

/// Returns true if path is an internal component to the MarFS filesystem refrence tree, including
/// the quota file MDAL_datasize, anything in the reference tree MDAL_reference and the
/// MDAL_subspaces directory itself.
pub fn is_internal(marfs_mdal_root: &str, path: &str) -> bool {
    let regex = RE.get_or_init(|| {
        // matches internal MarFS paths that have no mappings to the user tree
        Regex::new(&format!(
            "^{}/(MDAL_subspaces/[^/]+/)*MDAL_([^/]+|reference.*)$",
            marfs_mdal_root
        ))
        .unwrap()
    });

    regex.is_match(path)
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
        let j = (i + 2) as usize;

        if j >= path_vec.len() {
            break;
        }

        if path_vec[j] == "MDAL_subspaces" {
            i += 2;
            continue;
        } else {
            break;
        }
    }

    // i is index of last MDAL_subspaces, or -1 if none were found
    if i < 0 {
        path_vec.join("/")
    } else {
        // drop all MDAL_subspaces that are part of the MarFS namespae heirarchy including the lowest namespace but not including any paths the user created below that
        let subspace_heirarchy = path_vec[..(i as usize) + 1].to_vec();
        let subspaces_str = subspace_heirarchy
            .into_iter()
            .filter(|s| *s != "MDAL_subspaces")
            .collect::<Vec<&str>>()
            .join("/");

        format!("{}/{}", subspaces_str, path_vec[i as usize + 1..].join("/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all() {
        assert_eq!(internal_to_user("/marfs", ""), "/marfs");
        assert_eq!(internal_to_user("/marfs", "a"), "/marfs/a");
        assert_eq!(internal_to_user("/marfs", "a/b"), "/marfs/a/b");
        assert_eq!(internal_to_user("/marfs", "MDAL_subspaces/a"), "/marfs/a");
        assert_eq!(
            internal_to_user("/marfs", "MDAL_subspaces/a/file"),
            "/marfs/a/file"
        );
        assert_eq!(
            internal_to_user("/marfs", "MDAL_subspaces/a/dir/file"),
            "/marfs/a/dir/file"
        );
        assert_eq!(
            internal_to_user("/marfs", "MDAL_subspaces/a/MDAL_subspaces/b"),
            "/marfs/a/b"
        );
        assert_eq!(
            internal_to_user("/marfs", "MDAL_subspaces/a/MDAL_subspaces/b/dir"),
            "/marfs/a/b/dir"
        );
        assert_eq!(
            internal_to_user("/marfs", "MDAL_subspaces/a/MDAL_subspaces/b/dir/file"),
            "/marfs/a/b/dir/file"
        );

        // dumb user creates MDAL_subspaces cases
        assert_eq!(
            internal_to_user(
                "/marfs",
                "MDAL_subspaces/a/MDAL_subspaces/b/dir/MDAL_subspaces"
            ),
            "/marfs/a/b/dir/MDAL_subspaces"
        );
        assert_eq!(
            internal_to_user(
                "/marfs",
                "MDAL_subspaces/a/MDAL_subspaces/b/dir/MDAL_subspaces/file"
            ),
            "/marfs/a/b/dir/MDAL_subspaces/file"
        );
        assert_eq!(
            internal_to_user(
                "/marfs",
                "MDAL_subspaces/a/MDAL_subspaces/b/dir1/dir2/MDAL_subspaces"
            ),
            "/marfs/a/b/dir1/dir2/MDAL_subspaces"
        );
        assert_eq!(
            internal_to_user(
                "/marfs",
                "MDAL_subspaces/a/MDAL_subspaces/b/dir1/dir2/MDAL_subspaces/file"
            ),
            "/marfs/a/b/dir1/dir2/MDAL_subspaces/file"
        );
    }
}
