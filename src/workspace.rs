use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspacePaths {
    root: PathBuf,
}

impl WorkspacePaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn discover() -> anyhow::Result<Self> {
        let root = std::env::current_dir()?;
        Ok(Self::new(root))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn projects_dir(&self) -> PathBuf {
        self.root.join("projects")
    }

    pub fn android_dir(&self) -> PathBuf {
        self.root.join("android")
    }

    pub fn builtin_wasm(&self) -> PathBuf {
        self.root
            .join("assets")
            .join("builtins")
            .join("souprune_builtins.wasm")
    }

    pub fn apk_path(&self) -> PathBuf {
        self.android_dir()
            .join("souprune")
            .join("build")
            .join("outputs")
            .join("apk")
            .join("debug")
            .join("souprune-debug.apk")
    }
}
