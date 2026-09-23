//! Durable lifecycle belongs to Foundation; this module only dispatches product commands to devices.
use crate::{
    crypto::{SecretBox, constant_time_equal_32},
    db,
    error::{AppError, AppResult},
};
use chrono::{DateTime, Local, NaiveDate, TimeDelta, TimeZone, Utc};
pub use sarmg_operations::OperationState;
use sarmg_operations::{
    EnqueueOutcome, NewOperation, SqliteOperationStore, StoredOperation, Transition,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use std::{sync::Arc, time::Duration};
use sunshine_client_protocol::{
    Binding, Capabilities, Command, ConfigSnapshot, PROTOCOL, Permission, Report, Task,
};
use tokio::sync::{Notify, watch};
use uuid::Uuid;

const RESTART_EXECUTION_WINDOW_MICROS: i64 = 15 * 60 * 1_000_000;

#[derive(Clone)]
pub struct OperationManager {
    pub pool: sqlx::SqlitePool,
    pub store: SqliteOperationStore,
    secrets: SecretBox,
    notify: Arc<Notify>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    actor: String,
    request_ciphertext: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    binding: Binding,
    command: Command,
}
#[derive(Serialize)]
pub struct OperationView {
    pub operation_id: String,
    pub device_id: String,
    pub action: String,
    pub state: OperationState,
    pub attempt: i64,
    pub created_at_micros: i64,
    pub created_at_server: String,
    pub updated_at_micros: i64,
    pub result: Option<Report>,
    pub reconciliation: Option<Report>,
    pub resolution: Option<String>,
    pub status_reason: Option<&'static str>,
}

#[derive(Serialize)]
pub struct OperationSummary {
    pub blocking_count: i64,
}

fn server_timestamp(micros: i64) -> AppResult<String> {
    let utc = DateTime::<Utc>::from_timestamp_micros(micros)
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("任务创建时间无效")))?;
    Ok(utc
        .with_timezone(&Local)
        .format("%Y-%m-%d %H:%M:%S%.6f %:z")
        .to_string())
}

pub fn server_date_bounds(date: &str) -> AppResult<(i64, i64)> {
    let bytes = date.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| index != 4 && index != 7 && !byte.is_ascii_digit())
    {
        return Err(AppError::BadRequest("无效的服务器日期".into()));
    }
    let day = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest("无效的服务器日期".into()))?;
    let next = day
        .succ_opt()
        .ok_or_else(|| AppError::BadRequest("无效的服务器日期".into()))?;
    fn start(day: NaiveDate) -> AppResult<i64> {
        let midnight = day
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| AppError::BadRequest("无效的服务器日期".into()))?;
        // Some time zones advance at midnight. Find the first valid local time,
        // including the following midnight when a whole calendar day is skipped.
        for second in 0..=86_400 {
            let local_time = midnight + TimeDelta::seconds(second);
            match Local.from_local_datetime(&local_time) {
                chrono::LocalResult::Single(value) => return Ok(value.timestamp_micros()),
                chrono::LocalResult::Ambiguous(first, second) => {
                    return Ok(first.timestamp_micros().min(second.timestamp_micros()));
                }
                chrono::LocalResult::None => {}
            }
        }
        Err(AppError::BadRequest("无法解析服务器日期".into()))
    }
    Ok((start(day)?, start(next)?))
}

#[derive(FromRow)]
struct OperationListRow {
    operation_id: String,
    target_key: String,
    action: String,
    state: String,
    attempt: i64,
    created_at_micros: i64,
    updated_at_micros: i64,
    result_payload: Option<Vec<u8>>,
    report_json: Option<String>,
    resolution_code: Option<String>,
    error_code: Option<String>,
}
fn namespace(device_id: &str, installation_id: &Uuid, resource: &str) -> String {
    format!("sunshine.client.v2.{device_id}.{installation_id}.{resource}")
}
fn resource(command: &Command) -> &'static str {
    if matches!(
        command,
        Command::ReadConfig {}
            | Command::ListApplications {}
            | Command::ListPairedClients {}
            | Command::ReadLogs { .. }
            | Command::ReadDiagnostics {}
            | Command::ReadVirtualInputStatus {}
            | Command::ReadServiceStatus {}
    ) {
        "read"
    } else {
        "write"
    }
}
pub fn action(command: &Command) -> &'static str {
    match command {
        Command::ReadConfig {} => "sunshine.config.read",
        Command::PatchConfig { .. } => "sunshine.config.patch",
        Command::Restart { .. } => "sunshine.restart",
        Command::ListApplications {} => "sunshine.applications.list",
        Command::SaveApplication { .. } => "sunshine.applications.save",
        Command::DeleteApplication { .. } => "sunshine.applications.delete",
        Command::CloseApplication { .. } => "sunshine.applications.close",
        Command::UploadCover { .. } => "sunshine.applications.cover.upload",
        Command::SubmitPairingPin { .. } => "sunshine.pairing.pin.submit",
        Command::ListPairedClients {} => "sunshine.pairing.clients.list",
        Command::SetPairedClientEnabled { .. } => "sunshine.pairing.clients.update",
        Command::UnpairClient { .. } => "sunshine.pairing.clients.unpair",
        Command::UnpairAllClients { .. } => "sunshine.pairing.clients.unpair_all",
        Command::ReadLogs { .. } => "sunshine.logs.read",
        Command::ReadDiagnostics {} => "sunshine.diagnostics.read",
        Command::ReadVirtualInputStatus {} => "sunshine.virtual_input.read",
        Command::RunMaintenance { .. } => "sunshine.maintenance.run",
        Command::ReadServiceStatus {} => "sunshine.service.read",
        Command::ControlService { .. } => "sunshine.service.control",
    }
}
fn permission(command: &Command) -> Permission {
    match command {
        Command::ReadConfig {} => Permission::ReadConfig,
        Command::PatchConfig { .. } => Permission::WriteConfig,
        Command::Restart { .. } => Permission::Restart,
        Command::ListApplications {} => Permission::ReadApplications,
        Command::SaveApplication { .. }
        | Command::DeleteApplication { .. }
        | Command::CloseApplication { .. }
        | Command::UploadCover { .. } => Permission::ManageApplications,
        Command::SubmitPairingPin { .. }
        | Command::ListPairedClients {}
        | Command::SetPairedClientEnabled { .. }
        | Command::UnpairClient { .. }
        | Command::UnpairAllClients { .. } => Permission::ManagePairing,
        Command::ReadLogs { .. }
        | Command::ReadDiagnostics {}
        | Command::ReadVirtualInputStatus {} => Permission::ReadDiagnostics,
        Command::RunMaintenance { .. } => Permission::MaintainSunshine,
        Command::ReadServiceStatus {} | Command::ControlService { .. } => {
            Permission::ControlService
        }
    }
}
fn internal(error: impl Into<anyhow::Error>) -> AppError {
    AppError::Internal(error.into())
}
fn payload(stored: &StoredOperation) -> AppResult<Payload> {
    serde_json::from_slice(&stored.request_payload).map_err(internal)
}
pub fn decode_task(secrets: &SecretBox, stored: &StoredOperation) -> AppResult<Task> {
    let payload = payload(stored)?;
    let plaintext = secrets.decrypt_operation_request(
        &stored.operation.operation_id,
        &stored.action,
        &payload.request_ciphertext,
    )?;
    if !constant_time_equal_32(
        &stored.operation.request_fingerprint,
        &secrets.operation_request_fingerprint(&plaintext),
    ) {
        return Err(AppError::Crypto);
    }
    let request: Request = serde_json::from_str(&plaintext).map_err(internal)?;
    let task = Task {
        protocol: PROTOCOL.into(),
        operation_id: stored.operation.operation_id.clone(),
        permission: permission(&request.command),
        binding: request.binding,
        command: request.command,
    };
    if action(&task.command) != stored.action
        || task.binding.device_id.to_string() != stored.operation.target_key
        || stored.operation.namespace
            != namespace(
                &stored.operation.target_key,
                &task.binding.installation_id,
                resource(&task.command),
            )
    {
        return Err(AppError::Crypto);
    }
    Ok(task)
}
pub fn validate_idempotency_key(key: &str) -> AppResult<()> {
    if key.is_empty()
        || key.len() > 128
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        Err(AppError::BadRequest(
            "必须提供 1–128 字符的 Idempotency-Key".into(),
        ))
    } else {
        Ok(())
    }
}
impl OperationManager {
    pub fn new(pool: sqlx::SqlitePool, secrets: SecretBox) -> Self {
        Self {
            store: SqliteOperationStore::new(pool.clone()),
            pool,
            secrets,
            notify: Arc::new(Notify::new()),
        }
    }
    pub async fn enqueue(
        &self,
        actor: &str,
        id: &str,
        key: &str,
        command: Command,
    ) -> AppResult<OperationView> {
        validate_idempotency_key(key)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let device: db::Device = sqlx::query_as("SELECT * FROM devices WHERE device_id=?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::NotFound("设备不存在".into()))?;
        if device.revoked_at_micros.is_some() {
            return Err(AppError::Conflict("设备凭据已撤销".into()));
        }
        // Resolve the identity in the same transaction to fence enrollment/revocation.
        let manager: String =
            sqlx::query_scalar("SELECT manager_id FROM manager_identity WHERE singleton=1")
                .fetch_one(&mut *tx)
                .await?;
        let binding = Binding {
            manager_id: Uuid::parse_str(&manager).map_err(internal)?,
            device_id: Uuid::parse_str(id).map_err(internal)?,
            installation_id: device
                .installation_id
                .as_deref()
                .and_then(|id| Uuid::parse_str(id).ok())
                .ok_or_else(|| AppError::Conflict("请先注册 Client".into()))?,
        };
        let capabilities: Capabilities = device
            .capabilities_json
            .as_deref()
            .ok_or_else(|| AppError::Conflict("Client 尚未上报管理能力".into()))
            .and_then(|value| serde_json::from_str(value).map_err(internal))?;
        let operation_id = format!("op_{}", Uuid::new_v4());
        let task = Task {
            protocol: PROTOCOL.into(),
            operation_id: operation_id.clone(),
            binding: binding.clone(),
            permission: permission(&command),
            command: command.clone(),
        };
        task.validate(&binding, &capabilities)
            .map_err(|_| AppError::BadRequest("业务指令不符合白名单、权限或修订要求".into()))?;
        let plaintext = serde_json::to_string(&Request {
            binding: binding.clone(),
            command,
        })
        .map_err(internal)?;
        let mut digest = Sha256::new();
        for part in [
            actor.as_bytes(),
            id.as_bytes(),
            action(&task.command).as_bytes(),
            self.secrets.operation_idempotency_key_hash(key).as_slice(),
        ] {
            digest.update((part.len() as u64).to_be_bytes());
            digest.update(part);
        }
        let payload = Payload {
            actor: actor.into(),
            request_ciphertext: self.secrets.encrypt_operation_request(
                &operation_id,
                action(&task.command),
                &plaintext,
            )?,
        };
        let now = db::now_micros()?;
        let value = NewOperation {
            operation_id,
            namespace: namespace(id, &binding.installation_id, resource(&task.command)),
            target_key: id.into(),
            action: action(&task.command).into(),
            idempotency_digest: digest.finalize().into(),
            request_fingerprint: self.secrets.operation_request_fingerprint(&plaintext),
            request_payload: serde_json::to_vec(&payload).map_err(internal)?,
            max_attempts: 1,
            not_before_micros: now,
            created_at_micros: now,
        };
        let stored = SqliteOperationStore::enqueue_in(&mut tx, value)
            .await
            .map_err(|error| match error {
                sarmg_operations::Error::IdempotencyConflict => {
                    AppError::Conflict("幂等键已用于不同指令".into())
                }
                other => internal(other),
            })?;
        tx.commit().await?;
        let stored = match stored {
            EnqueueOutcome::Created(stored) | EnqueueOutcome::Existing(stored) => stored,
        };
        self.notify.notify_waiters();
        self.view(stored).await
    }
    async fn view(&self, stored: StoredOperation) -> AppResult<OperationView> {
        let reconciliation: Option<String> =
            sqlx::query_scalar("SELECT report_json FROM client_observations WHERE operation_id=?")
                .bind(&stored.operation.operation_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(OperationView {
            operation_id: stored.operation.operation_id,
            device_id: stored.operation.target_key,
            action: stored.action,
            state: stored.operation.state,
            attempt: i64::from(stored.operation.attempt),
            created_at_micros: stored.created_at_micros,
            created_at_server: server_timestamp(stored.created_at_micros)?,
            updated_at_micros: stored.updated_at_micros,
            result: stored
                .result_payload
                .as_deref()
                .map(serde_json::from_slice)
                .transpose()
                .map_err(internal)?,
            reconciliation: reconciliation
                .as_deref()
                .map(serde_json::from_str)
                .transpose()
                .map_err(internal)?,
            resolution: stored.resolution_code,
            // Foundation error codes may contain internal diagnostics. Expose
            // only reasons with an explicit administrator-facing meaning.
            status_reason: match stored.operation.error_code.as_deref() {
                Some("authorization_rotated") => Some("authorization_rotated"),
                _ => None,
            },
        })
    }
    pub async fn get_for_actor(&self, actor: &str, id: &str) -> AppResult<OperationView> {
        let stored = self
            .store
            .get(id)
            .await
            .map_err(internal)?
            .ok_or_else(|| AppError::NotFound("任务不存在".into()))?;
        if payload(&stored)?.actor != actor {
            return Err(AppError::Forbidden("不能访问其他管理员的任务".into()));
        }
        self.view(stored).await
    }
    pub async fn list_for_actor(&self, actor: &str, device: &str) -> AppResult<Vec<OperationView>> {
        self.list_for_actor_with_query(actor, device, None, false)
            .await
    }

    pub async fn list_for_actor_on_server_date(
        &self,
        actor: &str,
        device: &str,
        date: &str,
    ) -> AppResult<Vec<OperationView>> {
        let (from_micros, to_micros) = server_date_bounds(date)?;
        self.list_for_actor_with_query(actor, device, Some((from_micros, to_micros)), false)
            .await
    }

    pub async fn recent_for_actor(
        &self,
        actor: &str,
        device: &str,
    ) -> AppResult<Vec<OperationView>> {
        self.list_for_actor_with_query(actor, device, None, true)
            .await
    }

    async fn list_for_actor_with_query(
        &self,
        actor: &str,
        device: &str,
        range: Option<(i64, i64)>,
        recent: bool,
    ) -> AppResult<Vec<OperationView>> {
        // Read selected rows in one query. Full and dated logs have no row cap. Joining
        // observations avoids one operation and one reconciliation lookup per row.
        let projection = "SELECT op.operation_id,op.target_key,op.action,op.state,op.attempt, \
             op.created_at_micros,op.updated_at_micros,op.result_payload, \
             obs.report_json,op.resolution_code,op.error_code \
             FROM _sarmg_operations AS op \
             LEFT JOIN client_observations AS obs ON obs.operation_id=op.operation_id";
        let limit = if recent { "LIMIT 50" } else { "" };
        let time_filter = if range.is_some() {
            "AND op.created_at_micros>=? AND op.created_at_micros<?"
        } else {
            ""
        };
        let sql = format!(
            "{projection} WHERE op.target_key=? \
             AND json_extract(CAST(op.request_payload AS TEXT),'$.actor')=? \
             {time_filter} ORDER BY op.created_at_micros DESC,op.operation_id DESC {limit}"
        );
        let mut query = sqlx::query_as::<_, OperationListRow>(&sql)
            .bind(device)
            .bind(actor);
        if let Some((from_micros, to_micros)) = range {
            query = query.bind(from_micros).bind(to_micros);
        }
        let rows = query.fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(OperationView {
                    operation_id: row.operation_id,
                    device_id: row.target_key,
                    action: row.action,
                    state: OperationState::parse(&row.state).map_err(internal)?,
                    attempt: row.attempt,
                    created_at_micros: row.created_at_micros,
                    created_at_server: server_timestamp(row.created_at_micros)?,
                    updated_at_micros: row.updated_at_micros,
                    result: row
                        .result_payload
                        .as_deref()
                        .map(serde_json::from_slice)
                        .transpose()
                        .map_err(internal)?,
                    reconciliation: row
                        .report_json
                        .as_deref()
                        .map(serde_json::from_str)
                        .transpose()
                        .map_err(internal)?,
                    resolution: row.resolution_code,
                    status_reason: match row.error_code.as_deref() {
                        Some("authorization_rotated") => Some("authorization_rotated"),
                        _ => None,
                    },
                })
            })
            .collect()
    }

    pub async fn summary_for_device(&self, device: &str) -> AppResult<OperationSummary> {
        let blocking_count = sqlx::query_scalar(
            "SELECT COUNT(*) FROM _sarmg_operations WHERE target_key=? \
             AND state IN ('pending','running','unknown')",
        )
        .bind(device)
        .fetch_one(&self.pool)
        .await?;
        Ok(OperationSummary { blocking_count })
    }
    pub async fn resolve_for_actor(
        &self,
        actor: &str,
        id: &str,
        resolution: sarmg_operations::Resolution,
    ) -> AppResult<OperationView> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let bytes: Vec<u8> = sqlx::query_scalar(
            "SELECT request_payload FROM _sarmg_operations WHERE operation_id=?",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound("任务不存在".into()))?;
        if serde_json::from_slice::<Payload>(&bytes)
            .map_err(internal)?
            .actor
            != actor
        {
            return Err(AppError::Forbidden("不能处理其他管理员的任务".into()));
        }
        let updated = SqliteOperationStore::resolve_in(&mut tx, id, resolution, db::now_micros()?)
            .await
            .map_err(|_| AppError::Conflict("任务不需要人工核对".into()))?;
        db::audit(
            &mut tx,
            "operation.resolve",
            &updated.operation.target_key,
            actor,
            Some(&format!(
                "operation_id={id} resolution={}",
                resolution.as_str()
            )),
        )
        .await?;
        tx.commit().await?;
        self.view(updated).await
    }
    pub async fn recover_startup(&self) -> AppResult<u64> {
        sqlx::query("UPDATE devices SET session_id=NULL,last_seen_at_micros=NULL,health_at_micros=NULL,sunshine_reachable=NULL").execute(&self.pool).await?;
        let spaces: Vec<String> =
            sqlx::query_scalar("SELECT DISTINCT namespace FROM _sarmg_operations")
                .fetch_all(&self.pool)
                .await?;
        let mut count = 0;
        for space in spaces {
            count += self
                .store
                .recover_running(&space, "manager_interrupted", db::now_micros()?)
                .await
                .map_err(internal)?;
        }
        Ok(count)
    }
    pub async fn next(&self, device: &str, owner: &str) -> AppResult<Option<StoredOperation>> {
        loop {
            let now = db::now_micros()?;
            // Linearize authorization and claim with revocation/session replacement. A task
            // already claimed before revocation can be in flight; revocation cannot undo it.
            let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
            let installation: String = sqlx::query_scalar("SELECT installation_id FROM devices WHERE device_id=? AND session_id=? AND revoked_at_micros IS NULL AND credential_hash IS NOT NULL")
                .bind(device).bind(owner).fetch_optional(&mut *tx).await?.ok_or(AppError::Unauthorized)?;
            let installation = Uuid::parse_str(&installation).map_err(internal)?;
            let already_running: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM _sarmg_operations WHERE target_key=? AND state='running')",
            )
            .bind(device)
            .fetch_one(&mut *tx)
            .await?;
            if already_running {
                tx.commit().await?;
                return Ok(None);
            }
            let mut claimed = SqliteOperationStore::claim_next_in(
                &mut tx,
                &namespace(device, &installation, "read"),
                owner,
                now,
                now + 120_000_000,
            )
            .await
            .map_err(internal)?;
            if claimed.is_none() {
                claimed = SqliteOperationStore::claim_next_in(
                    &mut tx,
                    &namespace(device, &installation, "write"),
                    owner,
                    now,
                    now + 120_000_000,
                )
                .await
                .map_err(internal)?;
            }
            tx.commit().await?;
            let Some(claimed) = claimed else {
                return Ok(None);
            };
            if resource(&self.task(&claimed)?.command) == "write"
                && claimed
                    .created_at_micros
                    .saturating_add(RESTART_EXECUTION_WINDOW_MICROS)
                    <= db::now_micros()?
            {
                let mut expired = self.pool.begin_with("BEGIN IMMEDIATE").await?;
                SqliteOperationStore::apply_transition_owned_in(
                    &mut expired,
                    &claimed.operation,
                    Transition::Fail {
                        code: "execution_deadline_expired".into(),
                        retryable: false,
                        retry_not_before_micros: db::now_micros()?,
                    },
                    None,
                    db::now_micros()?,
                )
                .await
                .map_err(internal)?;
                expired.commit().await?;
                continue;
            }
            return Ok(Some(claimed));
        }
    }
    pub async fn uncertain(
        &self,
        device: &str,
        installation: &Uuid,
    ) -> AppResult<Option<StoredOperation>> {
        let id:Option<String>=sqlx::query_scalar("SELECT operation_id FROM _sarmg_operations WHERE namespace=? AND state='unknown' ORDER BY created_at_micros LIMIT 1").bind(namespace(device, installation, "write")).fetch_optional(&self.pool).await?;
        match id {
            Some(id) => self.store.get(&id).await.map_err(internal),
            None => Ok(None),
        }
    }
    pub fn task(&self, stored: &StoredOperation) -> AppResult<Task> {
        decode_task(&self.secrets, stored)
    }
    pub async fn disconnected(&self, stored: &StoredOperation) -> AppResult<()> {
        if stored.operation.state == OperationState::Running {
            self.store
                .abandon_claim(&stored.operation, "client_disconnected", db::now_micros()?)
                .await
                .map_err(internal)?;
        }
        Ok(())
    }
    pub async fn complete(
        &self,
        stored: &StoredOperation,
        session: &str,
        report: Report,
    ) -> AppResult<()> {
        let task = self.task(stored)?;
        validate_report(&task.command, &report)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM devices WHERE device_id=? AND session_id=? AND revoked_at_micros IS NULL)").bind(&stored.operation.target_key).bind(session).fetch_one(&mut *tx).await?;
        if !valid {
            return Err(AppError::Unauthorized);
        }
        let bytes = serde_json::to_vec(&report).map_err(internal)?;
        if stored.operation.state == OperationState::Unknown {
            // Evidence only: never forge a human Foundation resolution.
            let state: String =
                sqlx::query_scalar("SELECT state FROM _sarmg_operations WHERE operation_id=?")
                    .bind(&stored.operation.operation_id)
                    .fetch_one(&mut *tx)
                    .await?;
            if state != "unknown" {
                return Err(AppError::Conflict("任务已由管理员处理".into()));
            }
            sqlx::query("INSERT INTO client_observations(operation_id,report_json,observed_at_micros) VALUES(?,?,?) ON CONFLICT(operation_id) DO UPDATE SET report_json=excluded.report_json,observed_at_micros=excluded.observed_at_micros")
                .bind(&stored.operation.operation_id).bind(std::str::from_utf8(&bytes).map_err(internal)?).bind(db::now_micros()?).execute(&mut *tx).await?;
            db::audit(
                &mut tx,
                "operation.reconciled",
                &stored.operation.target_key,
                "client",
                Some(&stored.operation.operation_id),
            )
            .await?;
            tx.commit().await?;
            return Ok(());
        } else {
            let transition = match &report {
                Report::ConfigRead { .. }
                | Report::ConfigSaved { .. }
                | Report::RestartAcknowledged { .. }
                | Report::ApplicationsRead { .. }
                | Report::ApplicationSaved { .. }
                | Report::ApplicationDeleted { .. }
                | Report::ApplicationClosed {}
                | Report::CoverUploaded { .. }
                | Report::PairingPinSubmitted {}
                | Report::PairedClientsRead { .. }
                | Report::PairedClientUpdated { .. }
                | Report::LogsRead { .. }
                | Report::DiagnosticsRead { .. }
                | Report::VirtualInputStatusRead { .. }
                | Report::MaintenanceCompleted { .. }
                | Report::ServiceStatusRead { .. }
                | Report::ServiceControlled { .. } => Transition::Succeed,
                Report::Unknown { .. } => Transition::MarkIndeterminate {
                    code: "client_uncertain".into(),
                },
                Report::Conflict { .. } | Report::Rejected { .. } => Transition::Fail {
                    code: if matches!(report, Report::Conflict { .. }) {
                        "configuration_conflict"
                    } else {
                        "client_rejected"
                    }
                    .into(),
                    retryable: false,
                    retry_not_before_micros: db::now_micros()?,
                },
            };
            SqliteOperationStore::apply_transition_owned_in(
                &mut tx,
                &stored.operation,
                transition,
                Some(&bytes),
                db::now_micros()?,
            )
            .await
            .map_err(internal)?;
        }
        match &report {
            Report::ConfigRead { snapshot } => {
                sqlx::query("UPDATE devices SET snapshot_json=?,configuration_state='pending_verification' WHERE device_id=?")
                    .bind(serde_json::to_string(snapshot).map_err(internal)?)
                    .bind(&stored.operation.target_key)
                    .execute(&mut *tx)
                    .await?;
            }
            Report::ConfigSaved { snapshot } | Report::RestartAcknowledged { snapshot } => {
                let state = if matches!(report, Report::ConfigSaved { .. }) {
                    "awaiting_restart"
                } else {
                    "pending_verification"
                };
                sqlx::query("UPDATE devices SET snapshot_json=?,saved_revision=?,configuration_state=? WHERE device_id=?").bind(serde_json::to_string(snapshot).map_err(internal)?).bind(&snapshot.revision).bind(state).bind(&stored.operation.target_key).execute(&mut *tx).await?;
            }
            _ => {}
        }
        tx.commit().await?;
        Ok(())
    }
    pub async fn deliver_outbox(&self) -> AppResult<u64> {
        let events = self
            .store
            .pending_audit_events(128)
            .await
            .map_err(internal)?;
        let mut count = 0;
        for event in events {
            let stored = self
                .store
                .get(&event.operation_id)
                .await
                .map_err(internal)?
                .ok_or_else(|| internal(anyhow::anyhow!("missing audited operation")))?;
            let mut tx = self.pool.begin().await?;
            sqlx::query("INSERT INTO audit_logs(action,target,detail,actor,created_at_micros,outbox_id) VALUES(?,?,?,?,?,?) ON CONFLICT(outbox_id) DO NOTHING")
                .bind(format!("{}.{}",stored.action,event.to_state)).bind(&stored.operation.target_key).bind(format!("operation_id={} from={} state={}",event.operation_id,event.from_state,event.to_state)).bind(payload(&stored)?.actor).bind(event.created_at_micros).bind(&event.event_id).execute(&mut *tx).await?;
            if SqliteOperationStore::mark_audit_delivered_in(
                &mut tx,
                &event.event_id,
                db::now_micros()?,
            )
            .await
            .map_err(internal)?
            {
                count += 1;
            }
            tx.commit().await?;
        }
        Ok(count)
    }
    pub async fn run_until(self, mut shutdown: watch::Receiver<bool>) -> Result<(), String> {
        loop {
            if *shutdown.borrow() {
                break;
            }
            self.deliver_outbox()
                .await
                .map_err(|_| "operation audit unavailable".to_owned())?;
            let spaces: Vec<String> = sqlx::query_scalar(
                "SELECT DISTINCT namespace FROM _sarmg_operations WHERE state='running'",
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|_| "operation database unavailable".to_owned())?;
            for space in spaces {
                self.store
                    .recover_expired(
                        &space,
                        db::now_micros().map_err(|_| "clock unavailable".to_owned())?,
                    )
                    .await
                    .map_err(|_| "operation recovery unavailable".to_owned())?;
            }
            tokio::select! {_ = sarmg_server_runtime::wait_for_shutdown(&mut shutdown)=>break,_=tokio::time::sleep(Duration::from_secs(1))=>{}}
        }
        Ok(())
    }
}
pub fn validate_snapshot(snapshot: &ConfigSnapshot) -> AppResult<()> {
    sunshine_client_protocol::validate_revision(&snapshot.revision)
        .map_err(|_| AppError::BadRequest("Client 配置修订无效".into()))?;
    if !sunshine_client_protocol::is_supported_sunshine_version(&snapshot.sunshine_version)
        || snapshot.fields.iter().any(|(key, value)| {
            !sunshine_client_protocol::config::contains_field(key) || value.len() > 512
        })
    {
        return Err(AppError::BadRequest("Client 配置响应无效".into()));
    }
    Ok(())
}
fn validate_report(command: &Command, report: &Report) -> AppResult<()> {
    sunshine_client_protocol::validate_report(command, report)
        .map_err(|_| AppError::BadRequest("Client 结果与业务指令不符".into()))
}
