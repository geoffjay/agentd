//! Filesystem path safety for the knowledgebase document store.
#![allow(dead_code)]
//!
//! All reads/writes go through [`safe_doc_path`] which enforces strict
//! path-traversal safety and returns a validated absolute path.
//!
//! # Security properties
//!
//! - `project_id` must be a valid UUID (no directory traversal via that axis).
//! - `rel_path` is validated lexically (no `canonicalize` — target may not
//!   exist and `canonicalize` follows symlinks).
//! - Any existing parent that is a symlink is rejected.
//! - Atomic writes via temp file + `rename`.

use crate::error::KnowledgeError;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

/// Maximum path depth (number of components, not including the root prefix).
const MAX_DEPTH: usize = 8;
/// Maximum length of a single path component.
const MAX_COMPONENT_LEN: usize = 255;

/// Validate `rel_path` against `root/<project_id>` and return the absolute
/// filesystem path to the document.
///
/// # Errors
///
/// Returns [`KnowledgeError::InvalidPath`] if any safety check fails.
pub fn safe_doc_path(
    root: &Path,
    project_id: &str,
    rel_path: &str,
) -> Result<PathBuf, KnowledgeError> {
    // 1. project_id must be a valid UUID.
    Uuid::parse_str(project_id)
        .map_err(|_| KnowledgeError::InvalidPath("project_id is not a valid UUID".to_string()))?;

    // 2. rel_path basic checks.
    if rel_path.is_empty() {
        return Err(KnowledgeError::InvalidPath("rel_path must not be empty".to_string()));
    }

    // 3. Must end in `.md`.
    if !rel_path.ends_with(".md") {
        return Err(KnowledgeError::InvalidPath("rel_path must have a .md extension".to_string()));
    }

    // 4. Build a Path from the rel_path and inspect its components.
    let rel = Path::new(rel_path);
    let mut depth = 0usize;

    for component in rel.components() {
        match component {
            // Absolute paths are rejected.
            Component::RootDir | Component::Prefix(_) => {
                return Err(KnowledgeError::InvalidPath(
                    "rel_path must be a relative path".to_string(),
                ));
            }
            // Current-dir (`.`) components are rejected.
            Component::CurDir => {
                return Err(KnowledgeError::InvalidPath(
                    "rel_path must not contain '.' components".to_string(),
                ));
            }
            // Parent-dir (`..`) components are rejected — the key traversal check.
            Component::ParentDir => {
                return Err(KnowledgeError::InvalidPath(
                    "rel_path must not contain '..' components".to_string(),
                ));
            }
            Component::Normal(name) => {
                let name_str = name.to_str().ok_or_else(|| {
                    KnowledgeError::InvalidPath(
                        "rel_path contains non-UTF-8 characters".to_string(),
                    )
                })?;

                // Reject NUL bytes.
                if name_str.contains('\0') {
                    return Err(KnowledgeError::InvalidPath(
                        "rel_path must not contain NUL bytes".to_string(),
                    ));
                }

                // Reject empty components.
                if name_str.is_empty() {
                    return Err(KnowledgeError::InvalidPath(
                        "rel_path must not contain empty components".to_string(),
                    ));
                }

                // Cap component length.
                if name_str.len() > MAX_COMPONENT_LEN {
                    return Err(KnowledgeError::InvalidPath(format!(
                        "rel_path component '{name_str}' exceeds maximum length of {MAX_COMPONENT_LEN}"
                    )));
                }

                // Portable character allowlist: alphanumeric, hyphen, underscore, dot, space.
                for ch in name_str.chars() {
                    if !ch.is_alphanumeric() && ch != '-' && ch != '_' && ch != '.' && ch != ' ' {
                        return Err(KnowledgeError::InvalidPath(format!(
                            "rel_path component '{name_str}' contains disallowed character '{ch}'"
                        )));
                    }
                }

                depth += 1;
                if depth > MAX_DEPTH {
                    return Err(KnowledgeError::InvalidPath(format!(
                        "rel_path exceeds maximum depth of {MAX_DEPTH}"
                    )));
                }
            }
        }
    }

    if depth == 0 {
        return Err(KnowledgeError::InvalidPath(
            "rel_path must have at least one component".to_string(),
        ));
    }

    // 5. Build the candidate absolute path.
    let project_dir = root.join(project_id);
    let candidate = project_dir.join(rel_path);

    // 6. Lexical containment check (no canonicalize).
    //    Normalize both sides by cleaning up redundant separators.
    let project_dir_str = project_dir.to_string_lossy();
    let candidate_str = candidate.to_string_lossy();
    if !candidate_str.starts_with(project_dir_str.as_ref()) {
        return Err(KnowledgeError::InvalidPath(
            "rel_path escapes the project directory".to_string(),
        ));
    }

    // 7. Reject any existing parent that is a symlink.
    let mut check = candidate.clone();
    // Walk from the candidate up to root, checking any existing prefix.
    loop {
        if check.exists() {
            let meta = std::fs::symlink_metadata(&check).map_err(|e| {
                KnowledgeError::Other(anyhow::anyhow!("stat failed for {}: {e}", check.display()))
            })?;
            if meta.file_type().is_symlink() {
                return Err(KnowledgeError::InvalidPath(format!(
                    "rel_path traverses a symlink at {}",
                    check.display()
                )));
            }
        }
        if !check.pop() {
            break;
        }
        if check == root {
            break;
        }
    }

    Ok(candidate)
}

/// Write `content` to `path` atomically using a sibling temp file + rename.
///
/// Creates parent directories as needed.
pub fn atomic_write(path: &Path, content: &[u8]) -> anyhow::Result<()> {
    let parent = path.parent().ok_or_else(|| anyhow::anyhow!("path has no parent"))?;
    std::fs::create_dir_all(parent)?;

    // Write to a temp file in the same directory to ensure same-filesystem rename.
    let tmp_path = parent.join(format!(".tmp-{}", uuid::Uuid::new_v4()));
    std::fs::write(&tmp_path, content)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tmp() -> TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    const PROJ: &str = "550e8400-e29b-41d4-a716-446655440000";

    #[test]
    fn test_valid_simple_path() {
        let dir = tmp();
        let path = safe_doc_path(dir.path(), PROJ, "notes.md").unwrap();
        assert!(path.ends_with("notes.md"));
    }

    #[test]
    fn test_valid_nested_path() {
        let dir = tmp();
        let path = safe_doc_path(dir.path(), PROJ, "docs/api/reference.md").unwrap();
        assert!(path.ends_with("reference.md"));
    }

    #[test]
    fn test_reject_parent_dir() {
        let dir = tmp();
        assert!(safe_doc_path(dir.path(), PROJ, "../escape.md").is_err());
    }

    #[test]
    fn test_reject_absolute_path() {
        let dir = tmp();
        assert!(safe_doc_path(dir.path(), PROJ, "/etc/passwd.md").is_err());
    }

    #[test]
    fn test_reject_non_md_extension() {
        let dir = tmp();
        assert!(safe_doc_path(dir.path(), PROJ, "notes.txt").is_err());
    }

    #[test]
    fn test_reject_no_extension() {
        let dir = tmp();
        assert!(safe_doc_path(dir.path(), PROJ, "notes").is_err());
    }

    #[test]
    fn test_reject_empty_rel_path() {
        let dir = tmp();
        assert!(safe_doc_path(dir.path(), PROJ, "").is_err());
    }

    #[test]
    fn test_reject_dot_component() {
        let dir = tmp();
        assert!(safe_doc_path(dir.path(), PROJ, "./notes.md").is_err());
    }

    #[test]
    fn test_reject_invalid_project_id() {
        let dir = tmp();
        assert!(safe_doc_path(dir.path(), "not-a-uuid", "notes.md").is_err());
    }

    #[test]
    fn test_reject_too_deep() {
        let dir = tmp();
        let deep = "a/b/c/d/e/f/g/h/notes.md"; // 9 components — exceeds MAX_DEPTH=8
        assert!(safe_doc_path(dir.path(), PROJ, deep).is_err());
    }

    #[test]
    fn test_max_depth_exactly() {
        let dir = tmp();
        let at_limit = "a/b/c/d/e/f/g/notes.md"; // 8 components — exactly MAX_DEPTH
        assert!(safe_doc_path(dir.path(), PROJ, at_limit).is_ok());
    }

    #[test]
    fn test_reject_disallowed_chars() {
        let dir = tmp();
        assert!(safe_doc_path(dir.path(), PROJ, "notes$(cmd).md").is_err());
    }

    #[test]
    fn test_reject_symlink_parent() {
        let dir = tmp();
        let real_dir = dir.path().join("real");
        std::fs::create_dir_all(&real_dir).unwrap();
        let link_path = dir.path().join(PROJ);
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real_dir, &link_path).unwrap();
        #[cfg(unix)]
        assert!(safe_doc_path(dir.path(), PROJ, "notes.md").is_err());
    }

    #[test]
    fn test_atomic_write_creates_file() {
        let dir = tmp();
        let target = dir.path().join("sub").join("test.md");
        atomic_write(&target, b"# Hello").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "# Hello");
    }
}
