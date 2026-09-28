//! Async VBoxManage client: detection plus thin, typed wrappers around command plans.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::cmd::{self, tokio_util_lite::CancellationToken, CommandOutput};
use crate::host::HostOs;
use crate::profile::VboxVersion;
use crate::vbox::parse::{self, VmInfo, VmListEntry};
use crate::vbox::plan::VboxCommand;
use crate::vbox::{
    guest_additions_iso_candidates, install_dir_of, vboxmanage_candidates, VirtualBoxInstall,
};
use crate::{CoreError, Result};

#[derive(Clone, Debug)]
pub struct VBoxManage {
    pub path: PathBuf,
}

/// Detect an installed VirtualBox. Returns Ok(None) when not installed.
pub async fn detect(os: HostOs) -> Result<Option<VirtualBoxInstall>> {
    let Some(path) = cmd::find_program("VBoxManage", &vboxmanage_candidates(os)) else {
        return Ok(None);
    };
    let client = VBoxManage { path: path.clone() };
    let out = client
        .run_raw(&["--version".into()], Duration::from_secs(30), None)
        .await?;
    let version_raw = out.stdout.trim().to_string();
    let Some(version) = parse::parse_version(&out.stdout) else {
        return Err(CoreError::Parse(format!(
            "Unrecognized VBoxManage --version output: {version_raw}"
        )));
    };
    let install_dir = install_dir_of(&path);
    let mut guest_additions_iso = guest_additions_iso_candidates(os, &install_dir)
        .into_iter()
        .find(|p| p.is_file());
    if guest_additions_iso.is_none() {
        // Ask VirtualBox where its own Guest Additions ISO is.
        if let Ok(out) = client
            .run_raw(
                &["list".into(), "systemproperties".into()],
                Duration::from_secs(30),
                None,
            )
            .await
        {
            if let Some(line) = out
                .stdout
                .lines()
                .find_map(|l| l.strip_prefix("Default Guest Additions ISO:"))
            {
                let p = PathBuf::from(line.trim());
                if p.is_file() {
                    guest_additions_iso = Some(p);
                }
            }
        }
    }
    Ok(Some(VirtualBoxInstall {
        vboxmanage: path,
        install_dir,
        version,
        version_raw,
        guest_additions_iso,
    }))
}

impl VBoxManage {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub async fn run_raw(
        &self,
        args: &[String],
        timeout: Duration,
        cancel: Option<&CancellationToken>,
    ) -> Result<CommandOutput> {
        // `-q` suppresses the version banner on every invocation.
        let mut full = Vec::with_capacity(args.len() + 1);
        full.push("-q".to_string());
        full.extend_from_slice(args);
        let out = cmd::run(&self.path, &full, timeout, cancel, None).await?;
        if !out.success() {
            return Err(CoreError::VBoxManageFailed {
                args: args.to_vec(),
                code: out.code,
                stdout: out.stdout,
                stderr: out.stderr,
            });
        }
        Ok(out)
    }

    pub async fn run(
        &self,
        c: &VboxCommand,
        cancel: Option<&CancellationToken>,
    ) -> Result<CommandOutput> {
        tracing::info!(step = %c.description, args = ?c.args, "VBoxManage");
        self.run_raw(&c.args, Duration::from_secs(c.timeout_secs), cancel)
            .await
    }

    pub async fn version(&self) -> Result<VboxVersion> {
        let out = self
            .run_raw(&["--version".into()], Duration::from_secs(30), None)
            .await?;
        parse::parse_version(&out.stdout)
            .ok_or_else(|| CoreError::Parse("bad version output".into()))
    }

    pub async fn list_vms(&self) -> Result<Vec<VmListEntry>> {
        let out = self
            .run_raw(
                &["list".into(), "vms".into()],
                Duration::from_secs(60),
                None,
            )
            .await?;
        Ok(parse::parse_vm_list(&out.stdout))
    }

    pub async fn running_vms(&self) -> Result<Vec<VmListEntry>> {
        let out = self
            .run_raw(
                &["list".into(), "runningvms".into()],
                Duration::from_secs(60),
                None,
            )
            .await?;
        Ok(parse::parse_vm_list(&out.stdout))
    }

    pub async fn vm_info(&self, uuid_or_name: &str) -> Result<VmInfo> {
        let out = self
            .run(&super::plan::showvminfo(uuid_or_name), None)
            .await?;
        parse::parse_vm_info(&out.stdout)
    }

    pub async fn guest_properties(&self, uuid: &str) -> Result<BTreeMap<String, String>> {
        let out = self.run(&super::plan::guest_properties(uuid), None).await?;
        Ok(parse::parse_guest_properties(&out.stdout))
    }

    pub async fn extradata(&self, uuid: &str, key: &str) -> Result<Option<String>> {
        let out = self
            .run(&super::plan::get_extradata(uuid, key), None)
            .await?;
        Ok(parse::parse_extradata_get(&out.stdout))
    }

    pub async fn default_machine_folder(&self) -> Result<Option<PathBuf>> {
        let out = self
            .run_raw(
                &["list".into(), "systemproperties".into()],
                Duration::from_secs(30),
                None,
            )
            .await?;
        Ok(parse::parse_default_machine_folder(&out.stdout).map(PathBuf::from))
    }

    pub async fn hostinfo(&self) -> Result<parse::HostInfoOutput> {
        let out = self
            .run_raw(
                &["list".into(), "hostinfo".into()],
                Duration::from_secs(30),
                None,
            )
            .await?;
        Ok(parse::parse_hostinfo(&out.stdout))
    }

    /// Is this VM one we created? Checks the extra-data marker written at creation.
    pub async fn owned_by_app(&self, uuid: &str) -> Result<Option<String>> {
        self.extradata(uuid, super::EXTRADATA_INSTANCE_ID).await
    }

    /// Verify that a VM path is inside a base folder (used before deleting anything).
    pub fn path_within(path: &Path, base: &Path) -> bool {
        match (std::fs::canonicalize(path), std::fs::canonicalize(base)) {
            (Ok(p), Ok(b)) => p.starts_with(b),
            _ => path.starts_with(base),
        }
    }
}
