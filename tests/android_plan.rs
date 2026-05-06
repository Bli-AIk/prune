use std::path::PathBuf;

use prune::android::{AndroidPlan, AndroidProfile};

#[test]
fn android_plan_builds_assets_before_native_library() {
    let plan = AndroidPlan::new(
        PathBuf::from("/repo"),
        "mad_dummy_example".to_string(),
        vec![
            "undertale_preset".to_string(),
            "mad_dummy_example".to_string(),
        ],
        AndroidProfile::Release,
        None,
    );

    let labels: Vec<_> = plan
        .stages()
        .iter()
        .map(|stage| stage.label.as_str())
        .collect();

    assert_eq!(labels[0], "build builtin wasm");
    assert_eq!(labels[1], "build mod runtime undertale_preset");
    assert_eq!(labels[2], "build mod content undertale_preset");
    assert_eq!(labels[3], "build mod runtime mad_dummy_example");
    assert_eq!(labels[4], "build mod content mad_dummy_example");
    assert!(labels.contains(&"build android native library"));
    assert!(labels.contains(&"assemble debug apk"));
    assert!(labels.contains(&"install apk"));
    assert!(labels.contains(&"sync builtin wasm"));
}

#[test]
fn android_plan_syncs_dependency_mod_folders() {
    let plan = AndroidPlan::new(
        PathBuf::from("/repo"),
        "mad_dummy_example".to_string(),
        vec![
            "undertale_preset".to_string(),
            "mad_dummy_example".to_string(),
        ],
        AndroidProfile::Release,
        Some("DEVICE123".to_string()),
    );

    let rendered = plan.render_shell();

    assert!(rendered.contains(
        "adb -s DEVICE123 push /repo/projects/undertale_preset /sdcard/SoupRune/projects/"
    ));
    assert!(rendered.contains(
        "adb -s DEVICE123 push /repo/projects/mad_dummy_example /sdcard/SoupRune/projects/"
    ));
    assert!(rendered.contains("/repo/assets/builtins/souprune_builtins.wasm"));
}

#[test]
fn android_plan_uses_debug_native_library_for_debug_profile() {
    let plan = AndroidPlan::new(
        PathBuf::from("/repo"),
        "mad_dummy_example".to_string(),
        vec!["mad_dummy_example".to_string()],
        AndroidProfile::Debug,
        None,
    );

    let rendered = plan.render_shell();

    assert!(rendered.contains("/repo/target/aarch64-linux-android/debug/libsouprune.so"));
    assert!(!rendered.contains("/repo/target/aarch64-linux-android/release/libsouprune.so"));
}

#[test]
fn android_plan_prepares_device_storage_and_permissions() {
    let plan = AndroidPlan::new(
        PathBuf::from("/repo"),
        "mad_dummy_example".to_string(),
        vec!["mad_dummy_example".to_string()],
        AndroidProfile::Release,
        Some("DEVICE123".to_string()),
    );

    let rendered = plan.render_shell();

    assert!(rendered.contains(
        "adb -s DEVICE123 shell mkdir -p /sdcard/SoupRune/projects /sdcard/SoupRune/builtins"
    ));
    assert!(
        rendered.contains("adb -s DEVICE123 shell appops set com.bliaik.souprune MANAGE_EXTERNAL_STORAGE allow || true")
    );
    assert!(
        rendered.contains("adb -s DEVICE123 shell pm grant com.bliaik.souprune android.permission.READ_EXTERNAL_STORAGE || true")
    );
}

#[test]
fn android_plan_prepares_android_jni_library_directory() {
    let plan = AndroidPlan::new(
        PathBuf::from("/repo"),
        "mad_dummy_example".to_string(),
        vec!["mad_dummy_example".to_string()],
        AndroidProfile::Release,
        None,
    );

    let rendered = plan.render_shell();

    assert!(
        rendered.contains("mkdir -p /repo/android/app/src/main/jniLibs/arm64-v8a"),
        "{rendered}"
    );
}
