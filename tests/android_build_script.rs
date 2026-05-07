use std::fs;
use std::path::PathBuf;

#[test]
fn apk_only_build_script_targets_souprune_game_apk() {
    let script = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../android/build.sh"),
    )
    .expect("read android/build.sh");

    assert!(script.contains(":souprune:assembleDebug"));
    assert!(script.contains("souprune-debug.apk"));
    assert!(script.contains("android/souprune/src/main/jniLibs/arm64-v8a"));
    assert!(!script.contains(":prune:assembleDebug"));
}
