//! Product-owned management protocol. No streaming, arbitrary command, path or URL messages.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub mod config;

pub const PROTOCOL: &str = "sunshine-management/2";
/// A slash is not legal in an RFC 6455 WebSocket subprotocol header token.
pub const WEBSOCKET_SUBPROTOCOL: &str = "sunshine-management.v2";
pub const SUNSHINE_VERSION: &str = "2026.914.233613";
pub const SUPPORTED_SUNSHINE_VERSIONS: &[&str] = &[SUNSHINE_VERSION];
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024;

pub fn is_supported_sunshine_version(version: &str) -> bool {
    SUPPORTED_SUNSHINE_VERSIONS.contains(&version)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub manager_id: Uuid,
    pub device_id: Uuid,
    pub installation_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    ReadConfig,
    WriteConfig,
    Restart,
    ReadApplications,
    ManageApplications,
    ManagePairing,
    ReadDiagnostics,
    MaintainSunshine,
    ControlService,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestartPolicy {
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationRef {
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PreparationCommand {
    #[serde(rename = "do")]
    pub execute: String,
    pub undo: String,
    #[serde(default)]
    pub elevated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationSpec {
    pub name: String,
    #[serde(default)]
    pub output: String,
    #[serde(default)]
    pub cmd: String,
    #[serde(default, rename = "working-dir")]
    pub working_dir: String,
    #[serde(default, rename = "exclude-global-prep-cmd")]
    pub exclude_global_prep_cmd: bool,
    #[serde(default)]
    pub elevated: bool,
    #[serde(default, rename = "auto-detach")]
    pub auto_detach: bool,
    #[serde(default, rename = "wait-all")]
    pub wait_all: bool,
    #[serde(default = "default_exit_timeout", rename = "exit-timeout")]
    pub exit_timeout: u16,
    #[serde(default, rename = "prep-cmd")]
    pub prep_cmd: Vec<PreparationCommand>,
    #[serde(default)]
    pub detached: Vec<String>,
    #[serde(default, rename = "image-path")]
    pub image_path: String,
}

const fn default_exit_timeout() -> u16 {
    5
}

impl ApplicationSpec {
    pub fn has_host_commands(&self) -> bool {
        !self.output.is_empty()
            || !self.cmd.is_empty()
            || !self.working_dir.is_empty()
            || self.exclude_global_prep_cmd
            || self.elevated
            || !self.prep_cmd.is_empty()
            || !self.detached.is_empty()
    }

    pub fn validate(&self, host_commands_allowed: bool) -> Result<(), InvalidTask> {
        validate_text(&self.name, 1, 128)?;
        for value in [&self.output, &self.cmd, &self.working_dir, &self.image_path] {
            validate_optional_text(value, 4096)?;
        }
        if self.exit_timeout > 3600 || self.prep_cmd.len() > 16 || self.detached.len() > 16 {
            return Err(InvalidTask);
        }
        for prep in &self.prep_cmd {
            validate_optional_text(&prep.execute, 4096)?;
            validate_optional_text(&prep.undo, 4096)?;
            if prep.execute.is_empty() && prep.undo.is_empty() {
                return Err(InvalidTask);
            }
        }
        for command in &self.detached {
            validate_text(command, 1, 4096)?;
        }
        if self.has_host_commands() && !host_commands_allowed {
            return Err(InvalidTask);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationView {
    pub reference: ApplicationRef,
    pub specification: ApplicationSpec,
}

pub fn application_reference(application: &ApplicationSpec) -> Result<ApplicationRef, InvalidTask> {
    application.validate(true)?;
    let bytes = serde_json::to_vec(application).map_err(|_| InvalidTask)?;
    let mut digest = Sha256::new();
    digest.update(b"sunshine-application-v1\0");
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
    Ok(ApplicationRef {
        fingerprint: format!("{:x}", digest.finalize()),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationsSnapshot {
    pub revision: String,
    pub applications: Vec<ApplicationView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairedClient {
    pub uuid: String,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairedClientsSnapshot {
    pub revision: String,
    pub clients: Vec<PairedClient>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogCursor {
    pub revision: String,
    pub before_offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogPage {
    pub revision: String,
    pub text: String,
    pub start_offset: u64,
    pub end_offset: u64,
    pub total_bytes: u64,
    pub previous: Option<LogCursor>,
    pub redacted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DriverStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub required_version: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VirtualInputStatus {
    pub virtualhid: DriverStatus,
    pub vigembus: DriverStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    Running,
    Stopped,
    Starting,
    Stopping,
    Unavailable,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceAction {
    Start,
    Stop,
    Restart,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticSnapshot {
    pub sunshine_version: Option<String>,
    pub platform: Option<String>,
    pub api_reachable: bool,
    pub authentication_accepted: bool,
    pub service_state: ServiceState,
    pub configuration_revision: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceAction {
    ResetDisplayPersistence,
    ResetPortalToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    ReadConfig {},
    PatchConfig {
        expected_revision: String,
        set: BTreeMap<String, config::FieldValue>,
        remove: BTreeSet<String>,
        restart_policy: RestartPolicy,
    },
    Restart {
        expected_revision: String,
        /// An explicit administrator decision; the Client must ALSO allow restart locally.
        administrator_confirmed: bool,
    },
    ListApplications {},
    SaveApplication {
        expected_revision: String,
        target: Option<ApplicationRef>,
        application: ApplicationSpec,
        administrator_confirmed_host_commands: bool,
    },
    DeleteApplication {
        expected_revision: String,
        target: ApplicationRef,
        administrator_confirmed: bool,
    },
    CloseApplication {
        administrator_confirmed: bool,
    },
    UploadCover {
        key: String,
        png_base64: String,
        administrator_confirmed: bool,
    },
    SubmitPairingPin {
        pairing_id: String,
        pin: String,
        name: String,
    },
    ListPairedClients {},
    SetPairedClientEnabled {
        uuid: String,
        enabled: bool,
        administrator_confirmed: bool,
    },
    UnpairClient {
        uuid: String,
        administrator_confirmed: bool,
    },
    UnpairAllClients {
        administrator_confirmed: bool,
    },
    ReadLogs {
        cursor: Option<LogCursor>,
        limit_bytes: u32,
    },
    ReadDiagnostics {},
    ReadVirtualInputStatus {},
    RunMaintenance {
        action: MaintenanceAction,
        administrator_confirmed: bool,
    },
    ReadServiceStatus {},
    ControlService {
        action: ServiceAction,
        administrator_confirmed: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub protocol: String,
    pub operation_id: String,
    pub binding: Binding,
    pub permission: Permission,
    pub command: Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryMode {
    Execute,
    /// May inspect durable facts/actual state, but MUST NOT perform a side effect.
    InspectOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ManagerMessage {
    Task { mode: DeliveryMode, task: Box<Task> },
    Revoked {},
}

/// These are execution observations, NOT another persistent task lifecycle.
/// Manager maps observations into the existing Foundation operation transitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Report {
    ConfigRead {
        snapshot: ConfigSnapshot,
    },
    ConfigSaved {
        snapshot: ConfigSnapshot,
    },
    RestartAcknowledged {
        snapshot: ConfigSnapshot,
    },
    ApplicationsRead {
        snapshot: ApplicationsSnapshot,
    },
    ApplicationSaved {
        snapshot: ApplicationsSnapshot,
    },
    ApplicationDeleted {
        snapshot: ApplicationsSnapshot,
    },
    ApplicationClosed {},
    CoverUploaded {
        path: String,
    },
    PairingPinSubmitted {},
    PairedClientsRead {
        snapshot: PairedClientsSnapshot,
    },
    PairedClientUpdated {
        snapshot: PairedClientsSnapshot,
    },
    LogsRead {
        page: LogPage,
    },
    DiagnosticsRead {
        snapshot: DiagnosticSnapshot,
    },
    VirtualInputStatusRead {
        status: VirtualInputStatus,
    },
    MaintenanceCompleted {
        action: MaintenanceAction,
    },
    ServiceStatusRead {
        state: ServiceState,
    },
    ServiceControlled {
        action: ServiceAction,
        state: ServiceState,
    },
    Conflict {
        actual_revision: String,
    },
    Rejected {
        reason: Rejection,
    },
    Unknown {
        reason: Uncertainty,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rejection {
    InvalidTask,
    BindingMismatch,
    PermissionDenied,
    OperationIdReused,
    UnsupportedVersion,
    UnsafeConfiguration,
    SunshineUnavailable,
    JournalFull,
    ResourceConflict,
    UnsupportedCapability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Uncertainty {
    NoExecutionRecord,
    EffectNotConfirmed,
    RestartNotConfirmed,
    PersistenceFailure,
    SideEffectNotConfirmed,
    ServiceTransitionNotConfirmed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effectiveness {
    /// Reading a file does not prove that its settings are used by the running process.
    PendingVerification,
    AwaitingRestart,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigSnapshot {
    pub revision: String,
    pub sunshine_version: String,
    /// Only allowlisted fields; never send the entire local configuration to Manager.
    pub fields: BTreeMap<String, String>,
    pub effectiveness: Effectiveness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub protocol: String,
    pub client_version: String,
    pub os: ClientOs,
    pub sunshine_version: String,
    pub restart_allowed: bool,
    pub managed_fields: Vec<String>,
    pub application_management: bool,
    pub application_host_commands_allowed: bool,
    pub moonlight_pairing_management: bool,
    pub diagnostics: bool,
    pub maintenance: bool,
    pub service_control: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientOs {
    LinuxX86_64,
    WindowsX86_64,
    MacosX86_64,
    MacosAarch64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientMessage {
    Hello {
        binding: Binding,
        capabilities: Capabilities,
    },
    Heartbeat {
        sunshine_reachable: Option<bool>,
        configuration: Option<ConfigSnapshot>,
    },
    Result {
        operation_id: String,
        report: Report,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid sunshine management task")]
pub struct InvalidTask;

impl Task {
    pub fn validate(
        &self,
        binding: &Binding,
        capabilities: &Capabilities,
    ) -> Result<(), Rejection> {
        if &self.binding != binding {
            return Err(Rejection::BindingMismatch);
        }
        if self.protocol != PROTOCOL
            || self.binding.manager_id.is_nil()
            || self.binding.device_id.is_nil()
            || self.binding.installation_id.is_nil()
            || self.operation_id.strip_prefix("op_").is_none_or(|id| {
                !Uuid::parse_str(id).is_ok_and(|uuid| !uuid.is_nil() && uuid.to_string() == id)
            })
        {
            return Err(Rejection::InvalidTask);
        }
        let permission = match &self.command {
            Command::ReadConfig {} => Permission::ReadConfig,
            Command::PatchConfig {
                expected_revision,
                set,
                remove,
                ..
            } => {
                validate_revision(expected_revision).map_err(|_| Rejection::InvalidTask)?;
                if set.is_empty() && remove.is_empty()
                    || set.len() + remove.len() > config::FIELD_DEFINITIONS.len()
                {
                    return Err(Rejection::InvalidTask);
                }
                for (key, value) in set {
                    if remove.contains(key) || config::validate_field(key, value).is_err() {
                        return Err(Rejection::InvalidTask);
                    }
                }
                if remove.iter().any(|key| !config::contains_field(key)) {
                    return Err(Rejection::InvalidTask);
                }
                Permission::WriteConfig
            }
            Command::Restart {
                expected_revision,
                administrator_confirmed,
            } => {
                validate_revision(expected_revision).map_err(|_| Rejection::InvalidTask)?;
                if !capabilities.restart_allowed || !administrator_confirmed {
                    return Err(Rejection::PermissionDenied);
                }
                Permission::Restart
            }
            Command::ListApplications {} => require(
                capabilities.application_management,
                Permission::ReadApplications,
            )?,
            Command::SaveApplication {
                expected_revision,
                target,
                application,
                administrator_confirmed_host_commands,
            } => {
                require(
                    capabilities.application_management,
                    Permission::ManageApplications,
                )?;
                validate_revision(expected_revision).map_err(|_| Rejection::InvalidTask)?;
                if let Some(target) = target {
                    validate_revision(&target.fingerprint).map_err(|_| Rejection::InvalidTask)?;
                }
                application
                    .validate(capabilities.application_host_commands_allowed)
                    .map_err(|_| {
                        if application.has_host_commands() {
                            Rejection::UnsupportedCapability
                        } else {
                            Rejection::InvalidTask
                        }
                    })?;
                if application.has_host_commands() && !administrator_confirmed_host_commands {
                    return Err(Rejection::PermissionDenied);
                }
                Permission::ManageApplications
            }
            Command::DeleteApplication {
                expected_revision,
                target,
                administrator_confirmed,
            } => {
                require(
                    capabilities.application_management,
                    Permission::ManageApplications,
                )?;
                if !administrator_confirmed {
                    return Err(Rejection::PermissionDenied);
                }
                validate_revision(expected_revision).map_err(|_| Rejection::InvalidTask)?;
                validate_revision(&target.fingerprint).map_err(|_| Rejection::InvalidTask)?;
                Permission::ManageApplications
            }
            Command::CloseApplication {
                administrator_confirmed,
            } => {
                require(
                    capabilities.application_management,
                    Permission::ManageApplications,
                )?;
                if !administrator_confirmed {
                    return Err(Rejection::PermissionDenied);
                }
                Permission::ManageApplications
            }
            Command::UploadCover {
                key,
                png_base64,
                administrator_confirmed,
            } => {
                require(
                    capabilities.application_management,
                    Permission::ManageApplications,
                )?;
                if !administrator_confirmed {
                    return Err(Rejection::PermissionDenied);
                }
                validate_identifier(key, 64).map_err(|_| Rejection::InvalidTask)?;
                if png_base64.is_empty()
                    || png_base64.len() > 40 * 1024
                    || !png_base64
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
                {
                    return Err(Rejection::InvalidTask);
                }
                Permission::ManageApplications
            }
            Command::SubmitPairingPin {
                pairing_id,
                pin,
                name,
            } => {
                require(
                    capabilities.moonlight_pairing_management,
                    Permission::ManagePairing,
                )?;
                if pairing_id.len() != 32
                    || !pairing_id.bytes().all(|b| b.is_ascii_hexdigit())
                    || pin.len() != 4
                    || !pin.bytes().all(|b| b.is_ascii_digit())
                    || validate_text(name, 1, 128).is_err()
                {
                    return Err(Rejection::InvalidTask);
                }
                Permission::ManagePairing
            }
            Command::ListPairedClients {} => require(
                capabilities.moonlight_pairing_management,
                Permission::ManagePairing,
            )?,
            Command::SetPairedClientEnabled {
                uuid,
                enabled,
                administrator_confirmed,
            } => {
                require(
                    capabilities.moonlight_pairing_management,
                    Permission::ManagePairing,
                )?;
                if !enabled && !administrator_confirmed {
                    return Err(Rejection::PermissionDenied);
                }
                validate_uuid(uuid).map_err(|_| Rejection::InvalidTask)?;
                Permission::ManagePairing
            }
            Command::UnpairClient {
                uuid,
                administrator_confirmed,
            } => {
                require(
                    capabilities.moonlight_pairing_management,
                    Permission::ManagePairing,
                )?;
                if !administrator_confirmed {
                    return Err(Rejection::PermissionDenied);
                }
                validate_uuid(uuid).map_err(|_| Rejection::InvalidTask)?;
                Permission::ManagePairing
            }
            Command::UnpairAllClients {
                administrator_confirmed,
            } => {
                require(
                    capabilities.moonlight_pairing_management,
                    Permission::ManagePairing,
                )?;
                if !administrator_confirmed {
                    return Err(Rejection::PermissionDenied);
                }
                Permission::ManagePairing
            }
            Command::ReadLogs {
                cursor,
                limit_bytes,
            } => {
                require(capabilities.diagnostics, Permission::ReadDiagnostics)?;
                if !(1024..=24 * 1024).contains(limit_bytes) {
                    return Err(Rejection::InvalidTask);
                }
                if let Some(cursor) = cursor {
                    validate_revision(&cursor.revision).map_err(|_| Rejection::InvalidTask)?;
                }
                Permission::ReadDiagnostics
            }
            Command::ReadDiagnostics {} | Command::ReadVirtualInputStatus {} => {
                require(capabilities.diagnostics, Permission::ReadDiagnostics)?
            }
            Command::RunMaintenance {
                administrator_confirmed,
                ..
            } => {
                require(capabilities.maintenance, Permission::MaintainSunshine)?;
                if !administrator_confirmed {
                    return Err(Rejection::PermissionDenied);
                }
                Permission::MaintainSunshine
            }
            Command::ReadServiceStatus {} => {
                require(capabilities.service_control, Permission::ControlService)?
            }
            Command::ControlService {
                administrator_confirmed,
                ..
            } => {
                require(capabilities.service_control, Permission::ControlService)?;
                if !administrator_confirmed {
                    return Err(Rejection::PermissionDenied);
                }
                Permission::ControlService
            }
        };
        if self.permission != permission {
            return Err(Rejection::PermissionDenied);
        }
        Ok(())
    }

    /// Stable across object/set ordering and retransmission mode. Binding and permission are covered.
    pub fn fingerprint(&self) -> Result<String, InvalidTask> {
        let bytes = serde_json::to_vec(self).map_err(|_| InvalidTask)?;
        if bytes.len() > MAX_MESSAGE_BYTES {
            return Err(InvalidTask);
        }
        Ok(format!("{:x}", Sha256::digest(bytes)))
    }
}

fn require(condition: bool, permission: Permission) -> Result<Permission, Rejection> {
    if condition {
        Ok(permission)
    } else {
        Err(Rejection::UnsupportedCapability)
    }
}

fn validate_text(value: &str, minimum: usize, maximum: usize) -> Result<(), InvalidTask> {
    let length = value.len();
    if length < minimum
        || length > maximum
        || value != value.trim()
        || value.chars().any(|c| c == '\0' || c.is_control())
    {
        Err(InvalidTask)
    } else {
        Ok(())
    }
}
fn validate_optional_text(value: &str, maximum: usize) -> Result<(), InvalidTask> {
    if value.is_empty() {
        Ok(())
    } else {
        validate_text(value, 1, maximum)
    }
}
fn validate_identifier(value: &str, maximum: usize) -> Result<(), InvalidTask> {
    if value.is_empty()
        || value.len() > maximum
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        Err(InvalidTask)
    } else {
        Ok(())
    }
}
fn validate_uuid(value: &str) -> Result<(), InvalidTask> {
    let parsed = Uuid::parse_str(value).map_err(|_| InvalidTask)?;
    if parsed.is_nil() || parsed.to_string() != value.to_ascii_lowercase() {
        Err(InvalidTask)
    } else {
        Ok(())
    }
}

pub fn validate_revision(revision: &str) -> Result<(), InvalidTask> {
    if revision.len() == 64
        && revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(InvalidTask)
    }
}

pub fn decode_manager_message(bytes: &[u8]) -> Result<ManagerMessage, InvalidTask> {
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(InvalidTask);
    }
    serde_json::from_slice(bytes).map_err(|_| InvalidTask)
}

pub fn validate_report(command: &Command, report: &Report) -> Result<(), InvalidTask> {
    let valid = match (command, report) {
        (Command::ReadConfig {}, Report::ConfigRead { snapshot }) => {
            validate_config_snapshot(snapshot, Effectiveness::PendingVerification).is_ok()
        }
        (Command::PatchConfig { .. }, Report::ConfigSaved { snapshot }) => {
            validate_config_snapshot(snapshot, Effectiveness::AwaitingRestart).is_ok()
        }
        (Command::Restart { .. }, Report::RestartAcknowledged { snapshot }) => {
            validate_config_snapshot(snapshot, Effectiveness::PendingVerification).is_ok()
        }
        (Command::ListApplications {}, Report::ApplicationsRead { snapshot })
        | (Command::SaveApplication { .. }, Report::ApplicationSaved { snapshot })
        | (Command::DeleteApplication { .. }, Report::ApplicationDeleted { snapshot }) => {
            validate_applications(snapshot).is_ok()
        }
        (Command::CloseApplication { .. }, Report::ApplicationClosed {}) => true,
        (Command::UploadCover { .. }, Report::CoverUploaded { path }) => {
            validate_text(path, 1, 4096).is_ok()
        }
        (Command::SubmitPairingPin { .. }, Report::PairingPinSubmitted {}) => true,
        (Command::ListPairedClients {}, Report::PairedClientsRead { snapshot })
        | (Command::SetPairedClientEnabled { .. }, Report::PairedClientUpdated { snapshot })
        | (Command::UnpairClient { .. }, Report::PairedClientUpdated { snapshot })
        | (Command::UnpairAllClients { .. }, Report::PairedClientUpdated { snapshot }) => {
            validate_paired_clients(snapshot).is_ok()
        }
        (Command::ReadLogs { .. }, Report::LogsRead { page }) => validate_log_page(page).is_ok(),
        (Command::ReadDiagnostics {}, Report::DiagnosticsRead { snapshot }) => {
            validate_diagnostics(snapshot).is_ok()
        }
        (Command::ReadVirtualInputStatus {}, Report::VirtualInputStatusRead { status }) => {
            validate_virtual_input(status).is_ok()
        }
        (
            Command::RunMaintenance { action, .. },
            Report::MaintenanceCompleted { action: observed },
        ) => action == observed,
        (Command::ReadServiceStatus {}, Report::ServiceStatusRead { .. }) => true,
        (
            Command::ControlService { action, .. },
            Report::ServiceControlled {
                action: observed, ..
            },
        ) => action == observed,
        (_, Report::Conflict { actual_revision }) => validate_revision(actual_revision).is_ok(),
        (_, Report::Rejected { .. } | Report::Unknown { .. }) => true,
        _ => false,
    };
    if !valid || serde_json::to_vec(report).map_err(|_| InvalidTask)?.len() > MAX_MESSAGE_BYTES {
        Err(InvalidTask)
    } else {
        Ok(())
    }
}

fn validate_config_snapshot(
    snapshot: &ConfigSnapshot,
    effectiveness: Effectiveness,
) -> Result<(), InvalidTask> {
    validate_revision(&snapshot.revision)?;
    if snapshot.effectiveness != effectiveness
        || !is_supported_sunshine_version(&snapshot.sunshine_version)
        || snapshot.fields.iter().any(|(key, value)| {
            value.len() > 512 || config::validate_snapshot_field(key, value).is_err()
        })
    {
        Err(InvalidTask)
    } else {
        Ok(())
    }
}

fn validate_applications(snapshot: &ApplicationsSnapshot) -> Result<(), InvalidTask> {
    validate_revision(&snapshot.revision)?;
    if snapshot.applications.len() > 256 {
        return Err(InvalidTask);
    }
    let mut references = BTreeSet::new();
    for app in &snapshot.applications {
        validate_revision(&app.reference.fingerprint)?;
        app.specification.validate(true)?;
        if !references.insert(&app.reference.fingerprint) {
            return Err(InvalidTask);
        }
    }
    Ok(())
}

fn validate_paired_clients(snapshot: &PairedClientsSnapshot) -> Result<(), InvalidTask> {
    validate_revision(&snapshot.revision)?;
    if snapshot.clients.len() > 512 {
        return Err(InvalidTask);
    }
    let mut ids = BTreeSet::new();
    for client in &snapshot.clients {
        validate_uuid(&client.uuid)?;
        validate_text(&client.name, 1, 128)?;
        if !ids.insert(&client.uuid) {
            return Err(InvalidTask);
        }
    }
    Ok(())
}

fn validate_log_page(page: &LogPage) -> Result<(), InvalidTask> {
    validate_revision(&page.revision)?;
    if page.text.len() > 24 * 1024
        || page.start_offset > page.end_offset
        || page.end_offset > page.total_bytes
        || page.end_offset - page.start_offset != page.text.len() as u64
    {
        return Err(InvalidTask);
    }
    if let Some(cursor) = &page.previous {
        validate_revision(&cursor.revision)?;
        if cursor.revision != page.revision || cursor.before_offset != page.start_offset {
            return Err(InvalidTask);
        }
    }
    Ok(())
}

fn validate_diagnostics(value: &DiagnosticSnapshot) -> Result<(), InvalidTask> {
    if let Some(version) = &value.sunshine_version
        && !is_supported_sunshine_version(version)
    {
        return Err(InvalidTask);
    }
    if let Some(platform) = &value.platform {
        validate_text(platform, 1, 64)?;
    }
    if let Some(revision) = &value.configuration_revision {
        validate_revision(revision)?;
    }
    Ok(())
}

fn validate_virtual_input(value: &VirtualInputStatus) -> Result<(), InvalidTask> {
    for driver in [&value.virtualhid, &value.vigembus] {
        for text in [&driver.version, &driver.required_version, &driver.error]
            .into_iter()
            .flatten()
        {
            validate_text(text, 1, 256)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod macos_platform_tests {
    use super::*;
    #[test]
    fn additive_platform_values_preserve_old_clients() {
        for os in [
            ClientOs::LinuxX86_64,
            ClientOs::WindowsX86_64,
            ClientOs::MacosX86_64,
            ClientOs::MacosAarch64,
        ] {
            let bytes = serde_json::to_vec(&os).unwrap();
            assert_eq!(serde_json::from_slice::<ClientOs>(&bytes).unwrap(), os);
        }
        assert_eq!(
            serde_json::to_string(&ClientOs::MacosAarch64).unwrap(),
            "\"macos_aarch64\""
        );
        assert_eq!(
            serde_json::to_string(&ClientOs::MacosX86_64).unwrap(),
            "\"macos_x86_64\""
        );
    }

    #[test]
    fn only_verified_stable_sunshine_versions_are_admitted() {
        assert!(is_supported_sunshine_version("2026.914.233613"));
        assert!(!is_supported_sunshine_version("2026.906.222525"));
        assert!(!is_supported_sunshine_version("2026.910.221003"));
        assert!(!is_supported_sunshine_version("invalid"));
    }
}

#[cfg(test)]
mod protocol_v2_tests {
    use super::*;

    fn binding() -> Binding {
        Binding {
            manager_id: Uuid::new_v4(),
            device_id: Uuid::new_v4(),
            installation_id: Uuid::new_v4(),
        }
    }

    fn capabilities() -> Capabilities {
        Capabilities {
            protocol: PROTOCOL.into(),
            client_version: "0.2.0".into(),
            os: ClientOs::LinuxX86_64,
            sunshine_version: SUNSHINE_VERSION.into(),
            restart_allowed: true,
            managed_fields: config::FIELD_DEFINITIONS
                .iter()
                .map(|field| field.key.to_owned())
                .collect(),
            application_management: true,
            application_host_commands_allowed: true,
            moonlight_pairing_management: true,
            diagnostics: true,
            maintenance: true,
            service_control: true,
        }
    }

    fn application() -> ApplicationSpec {
        ApplicationSpec {
            name: "Steam".into(),
            output: String::new(),
            cmd: String::new(),
            working_dir: String::new(),
            exclude_global_prep_cmd: false,
            elevated: false,
            auto_detach: false,
            wait_all: false,
            exit_timeout: 5,
            prep_cmd: vec![],
            detached: vec![],
            image_path: String::new(),
        }
    }

    #[test]
    fn old_capability_shape_and_protocol_are_rejected() {
        let old = serde_json::json!({"protocol":"sunshine-management/1","client_version":"0.1.4","os":"linux_x86_64","sunshine_version":SUNSHINE_VERSION,"restart_allowed":true,"managed_fields":[]});
        assert!(serde_json::from_value::<Capabilities>(old).is_err());
        assert!(
            decode_manager_message(
                br#"{"type":"task","mode":"execute","task":{"protocol":"sunshine-management/1"}}"#
            )
            .is_err()
        );
    }

    #[test]
    fn host_commands_require_the_reported_capability_and_explicit_confirmation() {
        let binding = binding();
        let mut application = application();
        application.cmd = "steam".into();
        let mut task = Task {
            protocol: PROTOCOL.into(),
            operation_id: format!("op_{}", Uuid::new_v4()),
            binding: binding.clone(),
            permission: Permission::ManageApplications,
            command: Command::SaveApplication {
                expected_revision: "a".repeat(64),
                target: None,
                application,
                administrator_confirmed_host_commands: true,
            },
        };
        assert!(task.validate(&binding, &capabilities()).is_ok());
        let mut unavailable = capabilities();
        unavailable.application_host_commands_allowed = false;
        assert_eq!(
            task.validate(&binding, &unavailable),
            Err(Rejection::UnsupportedCapability)
        );
        if let Command::SaveApplication {
            administrator_confirmed_host_commands,
            ..
        } = &mut task.command
        {
            *administrator_confirmed_host_commands = false;
        }
        assert_eq!(
            task.validate(&binding, &capabilities()),
            Err(Rejection::PermissionDenied)
        );
    }

    #[test]
    fn reports_are_command_specific_and_bounded() {
        let command = Command::ReadLogs {
            cursor: None,
            limit_bytes: 24 * 1024,
        };
        let valid = Report::LogsRead {
            page: LogPage {
                revision: "b".repeat(64),
                text: "ready\n".into(),
                start_offset: 0,
                end_offset: 6,
                total_bytes: 6,
                previous: None,
                redacted: false,
            },
        };
        assert!(validate_report(&command, &valid).is_ok());
        assert!(validate_report(&Command::ReadDiagnostics {}, &valid).is_err());
        let oversized = Report::LogsRead {
            page: LogPage {
                revision: "b".repeat(64),
                text: "x".repeat(24 * 1024 + 1),
                start_offset: 0,
                end_offset: (24 * 1024 + 1) as u64,
                total_bytes: (24 * 1024 + 1) as u64,
                previous: None,
                redacted: false,
            },
        };
        assert!(validate_report(&command, &oversized).is_err());
    }

    #[test]
    fn application_references_are_content_bound() {
        let first = application_reference(&application()).unwrap();
        let mut renamed = application();
        renamed.name = "Steam Remote".into();
        assert_ne!(first, application_reference(&renamed).unwrap());
        assert_eq!(first, application_reference(&application()).unwrap());
    }
}
