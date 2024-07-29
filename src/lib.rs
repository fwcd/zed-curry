use std::{fs::{self, Permissions}, os::unix::fs::PermissionsExt};

use zed_extension_api::{self as zed, serde_json, settings::LspSettings};

struct CurryExtension {
    cached_binary_path: Option<String>,
}

impl CurryExtension {
    fn language_server_binary_path(
        &mut self,
        language_server_id: &zed::LanguageServerId,
    ) -> zed::Result<String> {
        if let Some(path) = &self.cached_binary_path {
            if fs::metadata(path).map_or(false, |stat| stat.is_file()) {
                return Ok(path.clone());
            }
        }

        zed::set_language_server_installation_status(
            &language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );
        let release = zed::latest_github_release(
            "fwcd/curry-language-server",
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false
            },
        )?;

        let (os, arch) = zed::current_platform();
        let suffix = match (os, arch) {
            (zed::Os::Windows, zed::Architecture::X8664) => "amd64-windows",
            (zed::Os::Mac, zed::Architecture::Aarch64) => "arm64-darwin",
            (zed::Os::Mac, zed::Architecture::X8664) => "x86_64-darwin",
            (zed::Os::Linux, zed::Architecture::X8664) => "x86_64-linux",
            _ => return Err(format!("The platform {os:?}/{arch:?} is not supported by curry-language-server")),
        };

        let asset_name = format!("curry-language-server-{suffix}.zip");
        let asset = release.assets
            .iter()
            .find(|asset| asset.name == asset_name)
            .ok_or_else(|| format!("Could not find asset {asset_name} in curry-language-server release"))?;

        let version_dir = format!("curry-language-server-{}", release.version);
        let binary_path = format!(
            "{version_dir}/bin/curry-language-server{extension}",
            extension = match os {
                zed::Os::Windows => ".exe",
                _ => "",
            }
        );

        if !fs::metadata(&binary_path).map_or(false, |stat| stat.is_file()) {
            zed::set_language_server_installation_status(
                &language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            zed::download_file(
                &asset.download_url,
                &version_dir,
                zed::DownloadedFileType::Zip
            )
            .map_err(|e| format!("Failed to download curry-language-server artifact: {e}"))?;

            // Mark the binary as executable since this mode seems to be gone after unzipping
            if matches!(os, zed::Os::Mac | zed::Os::Linux) {
                fs::set_permissions(&binary_path, Permissions::from_mode(0o755))
                    .map_err(|e| format!("Could not mark curry-language-server binary as executable: {e}"))?;
            }
        }

        self.cached_binary_path = Some(binary_path.clone());
        Ok(binary_path)
    }
}

impl zed::Extension for CurryExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        _worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        Ok(zed::Command {
            command: self.language_server_binary_path(language_server_id)?,
            args: vec![],
            env: Default::default(),
        })
    }

    fn language_server_workspace_configuration(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        let settings = LspSettings::for_worktree("curry-language-server", worktree)
            .ok()
            .and_then(|lsp_settings| lsp_settings.settings.clone())
            .unwrap_or_default();

        Ok(Some(serde_json::json!({
            "curry": settings
        })))
    }
}

zed::register_extension!(CurryExtension);
