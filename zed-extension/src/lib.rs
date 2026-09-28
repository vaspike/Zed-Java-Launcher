use zed_extension_api as zed;

struct JavaLauncherExtension;

impl zed::Extension for JavaLauncherExtension {
    fn new() -> Self {
        JavaLauncherExtension
    }

    fn run_slash_command(
        &self,
        command: zed::SlashCommand,
        _args: Vec<String>,
        _worktree: Option<&zed::Worktree>,
    ) -> Result<zed::SlashCommandOutput, String> {
        match command.name.as_str() {
            "launcher" => Ok(zed::SlashCommandOutput {
                sections: vec![zed::SlashCommandOutputSection {
                    range: (0usize..13usize).into(),
                    label: "Java Launcher".to_string(),
                }],
                text: "Java Launcher is ready.\n\nQuick Actions in Zed:\n  • Press Cmd+Shift+P -> search 'task: spawn' -> select '[Java Launcher] Open' to launch interactive TUI\n  • Press Cmd+Shift+P -> search 'task: spawn' -> select '[Java Launcher] Up <group>' to start a group\n  • Press Cmd+Shift+P -> search 'task: spawn' -> select '[Java Launcher] Down <group>' to stop a group\n  • Open Zed Debug Panel for single microservice Java debugging\n\nTerminal command:\n  java-launcher ui".to_string(),
            }),
            _ => Err("Unknown slash command".to_string()),
        }
    }
}

zed::register_extension!(JavaLauncherExtension);
