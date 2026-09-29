use std::fs;
use zed_extension_api::{
    self as zed, serde_json::Value, DebugAdapterBinary, DebugTaskDefinition,
    StartDebuggingRequestArguments, StartDebuggingRequestArgumentsRequest,
};

struct JavaLauncherExtension {
    cached_binary_path: Option<String>,
}

impl JavaLauncherExtension {
    fn get_or_download_binary(
        &mut self,
        user_provided_path: Option<String>,
        worktree: &zed::Worktree,
    ) -> Result<String, String> {
        // 1. User explicitly specified debug adapter path in settings
        if let Some(path) = user_provided_path {
            if !path.trim().is_empty() {
                return Ok(path);
            }
        }

        // 2. Bundled local binary (e.g., local dev extension package)
        if let Ok(curr) = std::env::current_dir() {
            let bundled = curr.join("bin/java-launcher");
            if bundled.is_file() {
                return Ok(bundled.to_string_lossy().to_string());
            }
        }

        // 3. Binary available in user's PATH
        if let Some(which) = worktree.which("java-launcher") {
            return Ok(which);
        }

        // 4. Cached binary path from current extension instance
        if let Some(path) = &self.cached_binary_path {
            if fs::metadata(path).is_ok_and(|m| m.is_file()) {
                return Ok(path.clone());
            }
        }

        // 5. Look for previously downloaded binary in extension working directory
        let (platform, _) = zed::current_platform();
        let ext = match platform {
            zed::Os::Windows => ".exe",
            _ => "",
        };

        if let Ok(entries) = fs::read_dir(".") {
            for entry in entries.filter_map(std::result::Result::ok) {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("java-launcher-") {
                    let candidates = [
                        entry.path().join(format!("java-launcher{ext}")),
                        entry.path().join(format!("bin/java-launcher{ext}")),
                        entry.path().join(format!("zed-java-launcher/bin/java-launcher{ext}")),
                    ];
                    for candidate in candidates {
                        if candidate.is_file() {
                            let path_str = candidate.to_string_lossy().to_string();
                            self.cached_binary_path = Some(path_str.clone());
                            return Ok(path_str);
                        }
                    }
                }
            }
        }

        // 6. Automatically download the native CLI binary from GitHub Releases
        let downloaded = self.download_binary()?;
        self.cached_binary_path = Some(downloaded.clone());
        Ok(downloaded)
    }

    fn download_binary(&mut self) -> Result<String, String> {
        let release = zed::latest_github_release(
            "vaspike/zed-java-launcher",
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )
        .map_err(|e| format!("Failed to fetch release info from GitHub: {e}"))?;

        let (platform, arch) = zed::current_platform();
        let ext = match platform {
            zed::Os::Windows => ".exe",
            _ => "",
        };

        let mut matching_assets: Vec<_> = release
            .assets
            .iter()
            .filter(|asset| {
                let name = asset.name.to_lowercase();
                let matches_os = match platform {
                    zed::Os::Mac => {
                        name.contains("darwin") || name.contains("macos") || name.contains("apple")
                    }
                    zed::Os::Linux => name.contains("linux"),
                    zed::Os::Windows => name.contains("windows") || name.ends_with(".zip"),
                };
                let matches_arch = match arch {
                    zed::Architecture::Aarch64 => {
                        name.contains("arm64") || name.contains("aarch64")
                    }
                    zed::Architecture::X8664 => {
                        name.contains("x86_64") || name.contains("x64") || name.contains("amd64")
                    }
                    zed::Architecture::X86 => name.contains("i686") || name.contains("x86"),
                };
                matches_os && matches_arch
            })
            .collect();

        matching_assets.sort_by_key(|asset| {
            if asset.name.starts_with("java-launcher-") {
                0
            } else {
                1
            }
        });

        let asset = matching_assets
            .first()
            .copied()
            .ok_or_else(|| {
                format!(
                    "No release asset found matching platform {:?} and architecture {:?}",
                    platform, arch
                )
            })?;

        let version_dir = format!("java-launcher-{}", release.version);
        fs::create_dir_all(&version_dir)
            .map_err(|e| format!("Failed to create directory {version_dir}: {e}"))?;

        let file_type = match platform {
            zed::Os::Windows => zed::DownloadedFileType::Zip,
            _ => {
                if asset.name.ends_with(".zip") {
                    zed::DownloadedFileType::Zip
                } else {
                    zed::DownloadedFileType::GzipTar
                }
            }
        };

        zed::download_file(&asset.download_url, &version_dir, file_type).map_err(|e| {
            format!(
                "Failed to download java-launcher from {}: {e}",
                asset.download_url
            )
        })?;

        let candidates = [
            format!("{version_dir}/java-launcher{ext}"),
            format!("{version_dir}/bin/java-launcher{ext}"),
            format!("{version_dir}/zed-java-launcher/bin/java-launcher{ext}"),
        ];

        let binary_path = candidates
            .into_iter()
            .find(|p| fs::metadata(p).is_ok_and(|m| m.is_file()))
            .ok_or_else(|| {
                format!("java-launcher binary not found in extracted archive in {version_dir}")
            })?;

        let _ = zed::make_file_executable(&binary_path);

        // Clean up older downloaded versions
        if let Ok(entries) = fs::read_dir(".") {
            for entry in entries.filter_map(std::result::Result::ok) {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("java-launcher-") && name != version_dir {
                        let _ = fs::remove_dir_all(path);
                    }
                }
            }
        }

        Ok(binary_path)
    }
}

impl zed::Extension for JavaLauncherExtension {
    fn new() -> Self {
        JavaLauncherExtension {
            cached_binary_path: None,
        }
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

        let binary_path = self.get_or_download_binary(user_provided_debug_adapter_path, worktree)?;

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

}

zed::register_extension!(JavaLauncherExtension);
