use std::path::PathBuf;

use crate::process::CommandStage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AndroidProfile {
    Debug,
    Release,
}

impl AndroidProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }
}

#[derive(Clone, Debug)]
pub struct AndroidPlan {
    root: PathBuf,
    mod_name: String,
    mod_order: Vec<String>,
    profile: AndroidProfile,
    device: Option<String>,
    stages: Vec<CommandStage>,
}

impl AndroidPlan {
    pub fn new(
        root: PathBuf,
        mod_name: String,
        mod_order: Vec<String>,
        profile: AndroidProfile,
        device: Option<String>,
    ) -> Self {
        let stages = build_stages(&root, &mod_name, &mod_order, profile, device.as_deref());
        Self {
            root,
            mod_name,
            mod_order,
            profile,
            device,
            stages,
        }
    }

    pub fn stages(&self) -> &[CommandStage] {
        &self.stages
    }

    pub fn render_shell(&self) -> String {
        self.stages
            .iter()
            .map(|stage| format!("# {}\n{}", stage.label, stage.render_shell()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn root(&self) -> &PathBuf {
        &self.root
    }

    pub fn mod_name(&self) -> &str {
        &self.mod_name
    }

    pub fn mod_order(&self) -> &[String] {
        &self.mod_order
    }

    pub fn profile(&self) -> AndroidProfile {
        self.profile
    }

    pub fn device(&self) -> Option<&str> {
        self.device.as_deref()
    }
}

fn build_stages(
    root: &PathBuf,
    mod_name: &str,
    mod_order: &[String],
    profile: AndroidProfile,
    device: Option<&str>,
) -> Vec<CommandStage> {
    let mut stages = Vec::new();

    stages.push(
        CommandStage::new(
            "build builtin wasm",
            "just",
            [if profile == AndroidProfile::Release {
                "builtin-build-release"
            } else {
                "builtin-build"
            }],
        )
        .cwd(root),
    );

    for mod_name in mod_order {
        stages.push(
            CommandStage::new(
                format!("build mod runtime {mod_name}"),
                "just",
                [runtime_recipe(profile), mod_name.as_str()],
            )
            .cwd(root),
        );
        stages.push(
            CommandStage::new(
                format!("build mod content {mod_name}"),
                "just",
                [content_recipe(profile), mod_name.as_str()],
            )
            .cwd(root),
        );
    }

    stages.push(
        CommandStage::new(
            "build android native library",
            "cargo",
            android_build_args(profile),
        )
        .cwd(root),
    );
    let jni_lib_dir = root
        .join("android")
        .join("souprune")
        .join("src")
        .join("main")
        .join("jniLibs")
        .join("arm64-v8a");
    stages.push(CommandStage::new(
        "prepare android native library directory",
        "mkdir",
        ["-p".to_string(), jni_lib_dir.display().to_string()],
    ));
    stages.push(CommandStage::new(
        "copy android native library",
        "cp",
        [
            root.join("target")
                .join("aarch64-linux-android")
                .join(native_artifact_dir(profile))
                .join("libsouprune.so")
                .display()
                .to_string(),
            jni_lib_dir.join("libsouprune.so").display().to_string(),
        ],
    ));
    stages.push(
        CommandStage::new(
            "assemble debug apk",
            "./gradlew",
            [":souprune:assembleDebug", "--no-daemon"],
        )
        .cwd(root.join("android")),
    );

    let apk = root
        .join("android")
        .join("souprune")
        .join("build")
        .join("outputs")
        .join("apk")
        .join("debug")
        .join("souprune-debug.apk");
    stages.push(adb_stage(
        "install apk",
        device,
        ["install", "-r", &apk.display().to_string()],
    ));

    if mod_order.last().map(String::as_str) != Some(mod_name) {
        stages.push(CommandStage::new(
            "warn active mod not last in dependency order",
            "printf",
            [format!(
                "active mod {mod_name} is not last in dependency order\n"
            )],
        ));
    }

    stages
}

fn runtime_recipe(profile: AndroidProfile) -> &'static str {
    match profile {
        AndroidProfile::Debug => "runtime-build",
        AndroidProfile::Release => "runtime-build-release",
    }
}

fn content_recipe(profile: AndroidProfile) -> &'static str {
    match profile {
        AndroidProfile::Debug => "content-build",
        AndroidProfile::Release => "content-build-release",
    }
}

fn android_build_args(profile: AndroidProfile) -> Vec<String> {
    let mut args = vec![
        "build".to_string(),
        "-p".to_string(),
        "souprune".to_string(),
        "--target".to_string(),
        "aarch64-linux-android".to_string(),
        "--features".to_string(),
        "android".to_string(),
    ];
    if profile == AndroidProfile::Release {
        args.push("--release".to_string());
    }
    args
}

fn native_artifact_dir(profile: AndroidProfile) -> &'static str {
    match profile {
        AndroidProfile::Debug => "debug",
        AndroidProfile::Release => "release",
    }
}

fn adb_stage(
    label: impl Into<String>,
    device: Option<&str>,
    args: impl IntoIterator<Item = impl AsRef<str>>,
) -> CommandStage {
    let mut full_args = Vec::new();
    if let Some(device) = device {
        full_args.push("-s".to_string());
        full_args.push(device.to_string());
    }
    full_args.extend(args.into_iter().map(|arg| arg.as_ref().to_owned()));
    CommandStage::new(label, "adb", full_args)
}
