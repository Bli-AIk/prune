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
    assert!(!labels.contains(&"sync builtin wasm"));
}

#[test]
fn android_plan_keeps_mod_sync_in_prune_provider_instead_of_adb_sdcard() {
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

    assert!(!rendered.contains("/sdcard/SoupRune"));
    assert!(!rendered.contains("adb -s DEVICE123 push /repo/projects"));
    assert!(!rendered.contains("souprune_builtins.wasm /sdcard"));
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
fn android_plan_does_not_request_external_storage_permissions() {
    let plan = AndroidPlan::new(
        PathBuf::from("/repo"),
        "mad_dummy_example".to_string(),
        vec!["mad_dummy_example".to_string()],
        AndroidProfile::Release,
        Some("DEVICE123".to_string()),
    );

    let rendered = plan.render_shell();

    assert!(!rendered.contains("MANAGE_EXTERNAL_STORAGE"));
    assert!(!rendered.contains("READ_EXTERNAL_STORAGE"));
    assert!(!rendered.contains("pm grant"));
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
        rendered.contains("mkdir -p /repo/android/souprune/src/main/jniLibs/arm64-v8a"),
        "{rendered}"
    );
    assert!(!rendered.contains("/repo/android/prune/src/main/jniLibs/arm64-v8a"));
}

#[test]
fn android_plan_installs_souprune_game_debug_apk() {
    let plan = AndroidPlan::new(
        PathBuf::from("/repo"),
        "mad_dummy_example".to_string(),
        vec!["mad_dummy_example".to_string()],
        AndroidProfile::Release,
        None,
    );

    let rendered = plan.render_shell();

    assert!(
        rendered.contains(
            "adb install -r /repo/android/souprune/build/outputs/apk/debug/souprune-debug.apk"
        ),
        "{rendered}"
    );
    assert!(
        rendered.contains("cd /repo/android && ./gradlew :souprune:assembleDebug --no-daemon"),
        "{rendered}"
    );
    assert!(!rendered.contains("/repo/android/prune/build/outputs/apk/debug/prune-debug.apk"));
}
