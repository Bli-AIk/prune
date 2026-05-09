use std::fs;

use prune::mods::{ActiveProjectConfig, ModGraph};

#[test]
fn active_project_config_reads_exact_mod_name_key() {
    let parsed = ActiveProjectConfig::parse(
        r#"
        # old_mod_name = "wrong"
        [project]
        mod_name = "mad_dummy_example"
        language = "en-US"
        "#,
    )
    .expect("config parses");

    assert_eq!(parsed.mod_name, "mad_dummy_example");
}

#[test]
fn dependency_order_includes_dependencies_before_active_mod() {
    let temp = tempfile::tempdir().expect("tempdir");
    let projects = temp.path();

    fs::create_dir_all(projects.join("undertale_preset")).unwrap();
    fs::write(
        projects.join("undertale_preset/mod.toml"),
        r#"
        name = "undertale_preset"
        version = "0.1.0"
        [dependencies]
        [mod_library]
        wasm = ".build/runtime.wasm"
        "#,
    )
    .unwrap();

    fs::create_dir_all(projects.join("mad_dummy_example")).unwrap();
    fs::write(
        projects.join("mad_dummy_example/mod.toml"),
        r#"
        name = "mad_dummy_example"
        version = "0.1.0"
        [dependencies]
        undertale_preset = "0.1.0"
        [mod_library]
        wasm = ".build/runtime.wasm"
        "#,
    )
    .unwrap();

    let graph = ModGraph::load(projects).expect("graph loads");
    let order = graph.dependency_order("mad_dummy_example").expect("order");

    assert_eq!(order, vec!["undertale_preset", "mad_dummy_example"]);
}
