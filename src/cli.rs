use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::android::{AndroidPlan, AndroidProfile};
use crate::mods::{ActiveProjectConfig, ModGraph};
use crate::workspace::WorkspacePaths;

#[derive(Parser, Debug)]
#[command(name = "prune", about = "SoupRune mod manager and build assistant")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Check the local SoupRune workspace.
    Doctor,
    /// Plan or run Android build and deploy stages.
    Android(AndroidCommand),
    /// Inspect and manage local mods.
    Mod(ModCommand),
}

#[derive(Args, Debug)]
pub struct AndroidCommand {
    #[command(subcommand)]
    pub action: AndroidAction,

    /// Override the active mod from projects/config.toml.
    #[arg(long = "mod", global = true)]
    pub mod_name: Option<String>,

    /// ADB device serial.
    #[arg(long, global = true)]
    pub device: Option<String>,

    /// Build profile for mod assets and native code.
    #[arg(long, value_enum, default_value_t = CliProfile::Release, global = true)]
    pub profile: CliProfile,
}

#[derive(Subcommand, Debug)]
pub enum AndroidAction {
    /// Check Android-related environment assumptions.
    Doctor,
    /// Print the build/deploy command stages without executing them.
    Plan,
    /// Execute the build/deploy stages.
    Deploy,
}

#[derive(Args, Debug)]
pub struct ModCommand {
    #[command(subcommand)]
    pub action: ModAction,
}

#[derive(Subcommand, Debug)]
pub enum ModAction {
    /// List discovered local mods.
    List,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CliProfile {
    Debug,
    Release,
}

impl From<CliProfile> for AndroidProfile {
    fn from(value: CliProfile) -> Self {
        match value {
            CliProfile::Debug => Self::Debug,
            CliProfile::Release => Self::Release,
        }
    }
}

pub fn run() -> Result<()> {
    if std::env::args().nth(1).as_deref() == Some("--gui") {
        return launch_gui();
    }

    run_cli(Cli::parse(), WorkspacePaths::discover()?)
}

pub fn run_cli(cli: Cli, workspace: WorkspacePaths) -> Result<()> {
    match cli.command {
        Commands::Doctor => {
            println!("workspace: {}", workspace.root().display());
            println!("projects: {}", workspace.projects_dir().display());
            println!("android: {}", workspace.android_dir().display());
            Ok(())
        }
        Commands::Android(command) => run_android(command, workspace),
        Commands::Mod(command) => run_mod(command, workspace),
    }
}

fn run_android(command: AndroidCommand, workspace: WorkspacePaths) -> Result<()> {
    match command.action {
        AndroidAction::Doctor => {
            println!("android dir: {}", workspace.android_dir().display());
            println!("apk: {}", workspace.apk_path().display());
            println!("builtin wasm: {}", workspace.builtin_wasm().display());
            Ok(())
        }
        AndroidAction::Plan | AndroidAction::Deploy => {
            let plan = build_android_plan(&command, &workspace)?;
            println!("{}", plan.render_shell());
            if matches!(command.action, AndroidAction::Deploy) {
                for stage in plan.stages() {
                    eprintln!("running: {}", stage.label);
                    stage.run()?;
                }
            }
            Ok(())
        }
    }
}

fn run_mod(command: ModCommand, workspace: WorkspacePaths) -> Result<()> {
    match command.action {
        ModAction::List => {
            let graph = ModGraph::load(workspace.projects_dir())?;
            for name in graph.manifests().keys() {
                println!("{name}");
            }
            Ok(())
        }
    }
}

fn build_android_plan(command: &AndroidCommand, workspace: &WorkspacePaths) -> Result<AndroidPlan> {
    let mod_name = match &command.mod_name {
        Some(mod_name) => mod_name.clone(),
        None => {
            let config_path = workspace.projects_dir().join("config.toml");
            ActiveProjectConfig::read(&config_path)
                .with_context(|| format!("read active mod from {}", config_path.display()))?
                .mod_name
        }
    };

    let graph = ModGraph::load(workspace.projects_dir())?;
    let mod_order = graph.dependency_order(&mod_name)?;

    Ok(AndroidPlan::new(
        PathBuf::from(workspace.root()),
        mod_name,
        mod_order,
        command.profile.into(),
        command.device.clone(),
    ))
}

#[cfg(feature = "gui")]
fn launch_gui() -> Result<()> {
    crate::gui::launch();
    Ok(())
}

#[cfg(not(feature = "gui"))]
fn launch_gui() -> Result<()> {
    anyhow::bail!("the Dioxus GUI is disabled; rebuild prune with --features gui")
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{AndroidAction, Cli, Commands};

    #[test]
    fn parses_android_plan_command() {
        let cli = Cli::parse_from([
            "prune",
            "android",
            "plan",
            "--mod",
            "mad_dummy_example",
            "--device",
            "DEVICE123",
        ]);

        match cli.command {
            Commands::Android(android) => {
                assert!(matches!(android.action, AndroidAction::Plan));
                assert_eq!(android.mod_name.as_deref(), Some("mad_dummy_example"));
                assert_eq!(android.device.as_deref(), Some("DEVICE123"));
            }
            _ => panic!("expected android command"),
        }
    }
}
