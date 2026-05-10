use clap::Parser;
use std::path::PathBuf;

use prune::cli::{Cli, Commands};

#[test]
fn server_command_defaults_to_souprune_build_script() {
    let cli = Cli::parse_from(["prune", "server", "--token", "test-token"]);

    match cli.command {
        Commands::Server(server) => {
            assert_eq!(server.build_script, PathBuf::from("android/build.sh"));
        }
        _ => panic!("expected server command"),
    }
}
