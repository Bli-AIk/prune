//! Project bundle archive creation for Android clients.
//! 面向 Android 客户端的项目包归档创建。

use std::fs::File;
use std::io::{self, Seek, Write};
use std::path::Path;

use anyhow::{Context, Result};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

pub fn create_projects_bundle(
    projects_root: impl AsRef<Path>,
    writer: impl Write + Seek,
) -> Result<()> {
    let projects_root = projects_root.as_ref();
    let mut zip = ZipWriter::new(writer);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let prefix = projects_root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("projects")
        .to_string();

    for entry in WalkDir::new(projects_root)
        .into_iter()
        .filter_entry(|entry| !should_skip(entry.path()))
    {
        let entry = entry.with_context(|| format!("walk {}", projects_root.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let relative = path
            .strip_prefix(projects_root)
            .with_context(|| format!("strip prefix from {}", path.display()))?;
        if should_skip(relative) {
            continue;
        }

        let archive_path = Path::new(&prefix).join(relative);
        zip.start_file_from_path(&archive_path, options)
            .with_context(|| format!("start zip entry {}", archive_path.display()))?;
        let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        io::copy(&mut file, &mut zip)
            .with_context(|| format!("write zip entry {}", archive_path.display()))?;
    }

    zip.finish().context("finish zip bundle")?;
    Ok(())
}

fn should_skip(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(".git" | ".build" | "target")
        )
    })
}
