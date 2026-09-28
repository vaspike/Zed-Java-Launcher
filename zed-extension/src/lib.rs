use zed_extension_api::{
    self as zed, serde_json::Value, DebugAdapterBinary, DebugTaskDefinition,
    StartDebuggingRequestArguments, StartDebuggingRequestArgumentsRequest,
};

struct JavaLauncherExtension;

impl zed::Extension for JavaLauncherExtension {
    fn new() -> Self {
        JavaLauncherExtension
    }

    fn get_dap_binary(
        &mut self,
        adapter_name: String,
        config: DebugTaskDefinition,
        user_provided_debug_adapter_path: Option<String>,
        worktree: &zed::Worktree,
    ) -> Result<DebugAdapterBinary, String> {
        if adapter_name != "java-launcher" {
            return Err(format!("Unsupported adapter \"{adapter_name}\""));
        }

        let binary_path = if let Some(path) = user_provided_debug_adapter_path {
            path
        } else if let Ok(curr) = std::env::current_dir() {
            let bundled = curr.join("bin/java-launcher");
            if bundled.exists() {
                bundled.to_string_lossy().to_string()
            } else if let Some(which) = worktree.which("java-launcher") {
                which
            } else {
                "java-launcher".to_string()
            }
        } else if let Some(which) = worktree.which("java-launcher") {
            which
        } else {
            "java-launcher".to_string()
        };

        let mut arguments = vec!["dap".to_string()];
        if let Ok(cfg) = zed::serde_json::from_str::<Value>(&config.config) {
            if let Some(group) = cfg.get("group").and_then(|v| v.as_str()) {
                arguments.push("--name".to_string());
                arguments.push(format!("Group: {group}"));
            } else if let Some(name) = cfg.get("name").and_then(|v| v.as_str()) {
                arguments.push("--name".to_string());
                arguments.push(name.to_string());
            } else if let Some(entry) = cfg.get("entry").and_then(|v| v.as_str()) {
                let label = entry.rsplit('.').next().unwrap_or(entry);
                arguments.push("--name".to_string());
                arguments.push(label.to_string());
            } else if let Some(label) = cfg.get("label").and_then(|v| v.as_str()) {
                arguments.push("--name".to_string());
                arguments.push(label.to_string());
            }
        }

        Ok(DebugAdapterBinary {
            command: Some(binary_path),
            arguments,
            envs: vec![],
            cwd: Some(worktree.root_path()),
            connection: None,
            request_args: StartDebuggingRequestArguments {
                request: StartDebuggingRequestArgumentsRequest::Launch,
                configuration: config.config,
            },
        })
    }

    fn dap_request_kind(
        &mut self,
        adapter_name: String,
        _config: Value,
    ) -> Result<StartDebuggingRequestArgumentsRequest, String> {
        if adapter_name != "java-launcher" {
            return Err(format!("Unsupported adapter \"{adapter_name}\""));
        }
        Ok(StartDebuggingRequestArgumentsRequest::Launch)
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
                text: "Java Launcher is ready.\n\nQuick Actions in Zed:\n  • In Zed Debug Panel: select '🚀 Group: <name>' to launch a group of microservices\n  • Press Cmd+Shift+P -> search 'task: spawn' -> select '[Java Launcher] Group up <group>' to start a group\n  • Press Cmd+Shift+P -> search 'task: spawn' -> select '[Java Launcher] Group down <group>' to stop a group".to_string(),
            }),
            _ => Err("Unknown slash command".to_string()),
        }
    }
}

zed::register_extension!(JavaLauncherExtension);
