use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use sha2::{Digest, Sha256};

use super::model::{Document, ProjectInfo, TreeNode, TreeNodeKind};

const IGNORED_DIRECTORIES: &[&str] = &[
    ".git",
    ".md-review",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    ".cache",
];
pub const MAX_DOCUMENT_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("project path is not a directory: {0}")]
    NotDirectory(String),
    #[error("invalid project-relative path")]
    InvalidPath,
    #[error("path is outside the project")]
    OutsideProject,
    #[error("not a Markdown file: {0}")]
    NotMarkdown(String),
    #[error("document is not valid UTF-8")]
    InvalidUtf8,
    #[error("file is too large ({size} bytes; maximum is {maximum} bytes): {path}")]
    TooLarge {
        path: String,
        size: u64,
        maximum: u64,
    },
}

#[derive(Debug)]
pub struct Project {
    root: PathBuf,
}

impl Project {
    pub fn open(path: PathBuf) -> Result<Self, ProjectError> {
        let root = path.canonicalize()?;
        if !root.is_dir() {
            return Err(ProjectError::NotDirectory(root.display().to_string()));
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn info(&self) -> ProjectInfo {
        ProjectInfo {
            name: self
                .root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Project")
                .to_owned(),
            root: self.root.display().to_string(),
        }
    }

    pub fn tree(&self) -> Result<Vec<TreeNode>, ProjectError> {
        Ok(self
            .scan_directory(&self.root, Path::new(""))?
            .unwrap_or_default())
    }

    pub fn document(&self, relative: &str) -> Result<Document, ProjectError> {
        if !is_markdown(Path::new(relative)) {
            return Err(ProjectError::NotMarkdown(relative.to_owned()));
        }
        let full_path = self.resolve(relative)?;
        self.ensure_size(&full_path, relative, MAX_DOCUMENT_BYTES)?;
        let bytes = fs::read(full_path)?;
        let content = String::from_utf8(bytes).map_err(|_| ProjectError::InvalidUtf8)?;
        let revision = revision(&content);
        Ok(Document {
            path: normalize_relative(Path::new(relative)),
            content,
            revision,
        })
    }

    pub fn asset(&self, relative: &str) -> Result<Vec<u8>, ProjectError> {
        let full_path = self.resolve(relative)?;
        self.ensure_size(&full_path, relative, MAX_DOCUMENT_BYTES)?;
        Ok(fs::read(full_path)?)
    }

    fn resolve(&self, relative: &str) -> Result<PathBuf, ProjectError> {
        let path = Path::new(relative);
        if path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(ProjectError::InvalidPath);
        }

        let candidate = self.root.join(path).canonicalize()?;
        if !candidate.starts_with(&self.root) {
            return Err(ProjectError::OutsideProject);
        }
        Ok(candidate)
    }

    fn ensure_size(&self, path: &Path, relative: &str, maximum: u64) -> Result<(), ProjectError> {
        let size = fs::metadata(path)?.len();
        if size > maximum {
            return Err(ProjectError::TooLarge {
                path: relative.to_owned(),
                size,
                maximum,
            });
        }
        Ok(())
    }

    fn scan_directory(
        &self,
        directory: &Path,
        relative: &Path,
    ) -> Result<Option<Vec<TreeNode>>, ProjectError> {
        let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.file_name().to_string_lossy().to_lowercase());

        let mut nodes = Vec::new();
        for entry in entries {
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue;
            }

            let name = entry.file_name().to_string_lossy().to_string();
            let child_relative = relative.join(&name);
            if file_type.is_dir() {
                if should_ignore_directory(&name) {
                    continue;
                }
                if let Some(children) = self.scan_directory(&entry.path(), &child_relative)?
                    && !children.is_empty()
                {
                    nodes.push(TreeNode {
                        name,
                        path: normalize_relative(&child_relative),
                        kind: TreeNodeKind::Directory,
                        children,
                    });
                }
            } else if file_type.is_file() && is_markdown(&entry.path()) {
                nodes.push(TreeNode {
                    name,
                    path: normalize_relative(&child_relative),
                    kind: TreeNodeKind::File,
                    children: Vec::new(),
                });
            }
        }

        Ok(Some(nodes))
    }
}

pub fn revision(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    format!("sha256:{}", hex::encode(digest))
}

fn should_ignore_directory(name: &str) -> bool {
    name.starts_with('.') || IGNORED_DIRECTORIES.contains(&name)
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("markdown")
        })
}

fn normalize_relative(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn tree_contains_only_markdown_and_ancestors() {
        let root = tempdir().unwrap();
        fs::create_dir_all(root.path().join("docs/nested")).unwrap();
        fs::create_dir_all(root.path().join("empty")).unwrap();
        fs::create_dir_all(root.path().join("node_modules/pkg")).unwrap();
        fs::write(root.path().join("README.md"), "# Hello").unwrap();
        fs::write(root.path().join("docs/nested/plan.markdown"), "Plan").unwrap();
        fs::write(root.path().join("empty/note.txt"), "skip").unwrap();
        fs::write(root.path().join("node_modules/pkg/hidden.md"), "skip").unwrap();

        let project = Project::open(root.path().to_path_buf()).unwrap();
        let tree = project.tree().unwrap();

        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].name, "docs");
        assert_eq!(tree[1].name, "README.md");
    }

    #[test]
    fn rejects_parent_paths() {
        let root = tempdir().unwrap();
        let project = Project::open(root.path().to_path_buf()).unwrap();
        assert!(matches!(
            project.document("../outside.md"),
            Err(ProjectError::InvalidPath)
        ));
    }

    #[test]
    fn rejects_markdown_files_over_the_size_limit() {
        let root = tempdir().unwrap();
        fs::write(
            root.path().join("huge.md"),
            vec![b'x'; MAX_DOCUMENT_BYTES as usize + 1],
        )
        .unwrap();
        let project = Project::open(root.path().to_path_buf()).unwrap();
        assert!(matches!(
            project.document("huge.md"),
            Err(ProjectError::TooLarge { .. })
        ));
    }
}
