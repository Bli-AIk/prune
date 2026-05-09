use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandStage {
    pub label: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub allow_failure: bool,
}

impl CommandStage {
    pub fn new(
        label: impl Into<String>,
        program: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            label: label.into(),
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            cwd: None,
            allow_failure: false,
        }
    }

    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn allow_failure(mut self) -> Self {
        self.allow_failure = true;
        self
    }

    pub fn run(&self) -> Result<()> {
        let mut command = Command::new(&self.program);
        command.args(&self.args);
        if let Some(cwd) = &self.cwd {
            command.current_dir(cwd);
        }

        let status = command
            .status()
            .with_context(|| format!("run stage {}", self.label))?;
        if !status.success() && !self.allow_failure {
            bail!("stage '{}' failed with status {status}", self.label);
        }
        Ok(())
    }

    pub fn render_shell(&self) -> String {
        let mut command = String::new();
        if let Some(cwd) = &self.cwd {
            command.push_str("cd ");
            command.push_str(&shell_arg(&cwd.display().to_string()));
            command.push_str(" && ");
        }
        command.push_str(&shell_arg(&self.program));
        for arg in &self.args {
            command.push(' ');
            command.push_str(&shell_arg(arg));
        }
        if self.allow_failure {
            command.push_str(" || true");
        }
        command
    }
}

fn shell_arg(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | ':' | '='))
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::CommandStage;

    #[test]
    fn required_stage_errors_on_nonzero_exit() {
        let stage = CommandStage::new("required", "false", std::iter::empty::<&str>());

        assert!(stage.run().is_err());
    }

    #[test]
    fn allow_failure_stage_does_not_error_on_nonzero_exit() {
        let stage =
            CommandStage::new("optional", "false", std::iter::empty::<&str>()).allow_failure();

        assert!(stage.run().is_ok());
        assert!(stage.render_shell().ends_with(" || true"));
    }
}
