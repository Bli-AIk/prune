use std::path::PathBuf;

use prune::workspace::WorkspacePaths;

#[test]
fn workspace_apk_path_points_at_souprune_game_debug_apk() {
    let workspace = WorkspacePaths::new(PathBuf::from("/repo"));

    assert_eq!(
        workspace.apk_path(),
        PathBuf::from("/repo/android/souprune/build/outputs/apk/debug/souprune-debug.apk")
    );
}
