use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveProjectConfig {
    pub mod_name: String,
    pub language: Option<String>,
}

impl ActiveProjectConfig {
    pub fn parse(input: &str) -> Result<Self> {
        let raw: RawProjectConfig = toml::from_str(input).context("parse projects/config.toml")?;
        Ok(Self {
            mod_name: raw.project.mod_name,
            language: raw.project.language,
        })
    }

    pub fn read(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let input = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        Self::parse(&input)
    }
}

#[derive(Deserialize)]
struct RawProjectConfig {
    project: RawProjectSection,
}

#[derive(Deserialize)]
struct RawProjectSection {
    mod_name: String,
    language: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ModManifest {
    pub name: String,
    pub version: Option<String>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    pub mod_library: Option<ModLibrary>,
    pub content_library: Option<ContentLibrary>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ModLibrary {
    pub wasm: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ContentLibrary {
    pub wasm: String,
}

impl ModManifest {
    pub fn parse(input: &str) -> Result<Self> {
        toml::from_str(input).context("parse mod.toml")
    }
}

#[derive(Clone, Debug, Default)]
pub struct ModGraph {
    manifests: BTreeMap<String, ModManifest>,
}

impl ModGraph {
    pub fn load(projects_root: impl AsRef<Path>) -> Result<Self> {
        let projects_root = projects_root.as_ref();
        let mut manifests = BTreeMap::new();

        for entry in fs::read_dir(projects_root)
            .with_context(|| format!("read projects root {}", projects_root.display()))?
        {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let manifest_path = entry.path().join("mod.toml");
            if !manifest_path.exists() {
                continue;
            }

            let input = fs::read_to_string(&manifest_path)
                .with_context(|| format!("read {}", manifest_path.display()))?;
            let manifest = ModManifest::parse(&input)
                .with_context(|| format!("parse {}", manifest_path.display()))?;
            manifests.insert(manifest.name.clone(), manifest);
        }

        Ok(Self { manifests })
    }

    pub fn dependency_order(&self, mod_name: &str) -> Result<Vec<String>> {
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut order = Vec::new();

        self.visit(mod_name, &mut visiting, &mut visited, &mut order)?;

        Ok(order)
    }

    pub fn manifests(&self) -> &BTreeMap<String, ModManifest> {
        &self.manifests
    }

    fn visit(
        &self,
        mod_name: &str,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
        order: &mut Vec<String>,
    ) -> Result<()> {
        if visited.contains(mod_name) {
            return Ok(());
        }
        if !visiting.insert(mod_name.to_owned()) {
            bail!("cyclic mod dependency involving {mod_name}");
        }

        let manifest = self
            .manifests
            .get(mod_name)
            .with_context(|| format!("unknown mod dependency {mod_name}"))?;

        for dependency in manifest.dependencies.keys() {
            self.visit(dependency, visiting, visited, order)?;
        }

        visiting.remove(mod_name);
        visited.insert(mod_name.to_owned());
        order.push(mod_name.to_owned());
        Ok(())
    }
}
