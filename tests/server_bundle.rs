use std::fs;
use std::io::{Cursor, Read};

use prune::server::create_projects_bundle;
use zip::ZipArchive;

#[test]
fn projects_bundle_contains_config_and_mod_manifests_without_git_metadata() {
    let temp = tempfile::tempdir().expect("tempdir");
    let projects = temp.path().join("projects");
    fs::create_dir_all(projects.join("mad_dummy_example/.git")).unwrap();
    fs::create_dir_all(projects.join("mad_dummy_example/assets")).unwrap();
    fs::write(
        projects.join("config.toml"),
        "[project]\nmod_name = \"mad_dummy_example\"\n",
    )
    .unwrap();
    fs::write(
        projects.join("mad_dummy_example/mod.toml"),
        "name = \"mad_dummy_example\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(projects.join("mad_dummy_example/.git/config"), "ignored").unwrap();
    fs::write(
        projects.join("mad_dummy_example/assets/readme.txt"),
        "asset",
    )
    .unwrap();

    let mut bundle = Cursor::new(Vec::new());
    create_projects_bundle(&projects, &mut bundle).expect("bundle created");

    let mut zip = ZipArchive::new(Cursor::new(bundle.into_inner())).expect("zip opens");
    let mut config = String::new();
    zip.by_name("projects/config.toml")
        .expect("config exists")
        .read_to_string(&mut config)
        .unwrap();
    assert!(config.contains("mad_dummy_example"));

    assert!(zip.by_name("projects/mad_dummy_example/mod.toml").is_ok());
    assert!(
        zip.by_name("projects/mad_dummy_example/assets/readme.txt")
            .is_ok()
    );
    assert!(
        zip.by_name("projects/mad_dummy_example/.git/config")
            .is_err()
    );
}
