use sunshine_client_protocol::{
    Capabilities, ClientOs, Command, PROTOCOL, Rejection, Report, SUNSHINE_VERSION, Uncertainty,
};
use sunshine_manager::{
    crypto::SecretBox,
    db,
    operations::{OperationManager, OperationState},
};
use uuid::Uuid;

async fn database() -> (tempfile::TempDir, sqlx::SqlitePool) {
    let dir = tempfile::tempdir().unwrap();
    let pool = db::open_or_initialize(&format!(
        "sqlite://{}",
        dir.path().join("state.db").display()
    ))
    .await
    .unwrap();
    (dir, pool)
}
fn secrets() -> SecretBox {
    SecretBox::new("test", [7; 32]).unwrap()
}

#[tokio::test]
async fn device_list_returns_every_instance_in_case_insensitive_name_order() {
    let (_dir, pool) = database().await;
    let secrets = secrets();
    for name in ["zulu", "Bravo", "alpha"] {
        db::create_device(&pool, &secrets, name, "admin")
            .await
            .unwrap();
    }
    let names = db::list_devices(&pool)
        .await
        .unwrap()
        .into_iter()
        .map(|device| device.name)
        .collect::<Vec<_>>();
    assert_eq!(names, ["alpha", "Bravo", "zulu"]);
}
async fn registered(pool: &sqlx::SqlitePool) -> (String, String) {
    let ticket = db::create_device(pool, &secrets(), "测试设备", "admin")
        .await
        .unwrap();
    let credential = db::random_token();
    db::enroll(
        pool,
        &ticket.device.id,
        Uuid::new_v4(),
        &ticket.token,
        &credential,
    )
    .await
    .unwrap();
    let capabilities = Capabilities {
        protocol: PROTOCOL.into(),
        client_version: "test".into(),
        os: ClientOs::LinuxX86_64,
        sunshine_version: SUNSHINE_VERSION.into(),
        restart_allowed: true,
        managed_fields: sunshine_client_protocol::config::FIELD_DEFINITIONS
            .iter()
            .map(|field| field.key.to_owned())
            .collect(),
        application_management: true,
        application_host_commands_allowed: true,
        moonlight_pairing_management: true,
        diagnostics: true,
        maintenance: true,
        service_control: true,
    };
    sqlx::query("UPDATE devices SET capabilities_json=? WHERE device_id=?")
        .bind(serde_json::to_string(&capabilities).unwrap())
        .bind(&ticket.device.id)
        .execute(pool)
        .await
        .unwrap();
    (ticket.device.id, credential)
}
async fn session(pool: &sqlx::SqlitePool, id: &str, session: &str) {
    sqlx::query("UPDATE devices SET session_id=?,last_seen_at_micros=? WHERE device_id=?")
        .bind(session)
        .bind(db::now_micros().unwrap())
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
}
fn manager(pool: &sqlx::SqlitePool) -> OperationManager {
    OperationManager::new(pool.clone(), SecretBox::new("test", [7; 32]).unwrap())
}

#[tokio::test]
async fn authorization_code_resolves_only_pending_active_instances() {
    let (_dir, pool) = database().await;
    let ticket = db::create_device(&pool, &secrets(), "pair", "admin")
        .await
        .unwrap();
    let target = db::resolve_pairing(&pool, &ticket.token).await.unwrap();
    assert_eq!(target["device_id"], ticket.device.id);
    assert_eq!(target["manager_id"], ticket.manager_id.to_string());
    assert!(db::resolve_pairing(&pool, "bad").await.is_err());
    assert!(
        db::resolve_pairing(&pool, &db::random_token())
            .await
            .is_err()
    );
    db::enroll(
        &pool,
        &ticket.device.id,
        Uuid::new_v4(),
        &ticket.token,
        &db::random_token(),
    )
    .await
    .unwrap();
    assert!(db::resolve_pairing(&pool, &ticket.token).await.is_err());
    let cancelled = db::create_device(&pool, &secrets(), "cancel", "admin")
        .await
        .unwrap();
    db::cancel_pairing(&pool, &cancelled.device.id, "admin")
        .await
        .unwrap();
    assert!(db::resolve_pairing(&pool, &cancelled.token).await.is_err());
    let revoked = db::create_device(&pool, &secrets(), "revoke", "admin")
        .await
        .unwrap();
    db::revoke(&pool, &revoked.device.id, "admin")
        .await
        .unwrap();
    assert!(db::resolve_pairing(&pool, &revoked.token).await.is_err());
}

#[tokio::test]
async fn rotating_instance_authorization_revokes_client_and_requires_new_code() {
    let (_dir, pool) = database().await;
    let secrets = secrets();
    let ticket = db::create_device(&pool, &secrets, "rotate", "admin")
        .await
        .unwrap();
    assert_eq!(
        db::get_authorization(&pool, &secrets, &ticket.device.id)
            .await
            .unwrap(),
        ticket.token
    );
    let encrypted: String =
        sqlx::query_scalar("SELECT authorization_code_enc FROM devices WHERE device_id=?")
            .bind(&ticket.device.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!encrypted.contains(&ticket.token));

    let installation = Uuid::new_v4();
    let old_credential = db::random_token();
    db::enroll(
        &pool,
        &ticket.device.id,
        installation,
        &ticket.token,
        &old_credential,
    )
    .await
    .unwrap();
    let new_code = db::random_authorization_code();
    db::rotate_authorization(&pool, &secrets, &ticket.device.id, &new_code, "admin")
        .await
        .unwrap();
    assert!(
        db::authenticate_device(&pool, &old_credential)
            .await
            .is_err()
    );
    assert!(db::resolve_pairing(&pool, &ticket.token).await.is_err());
    assert_eq!(
        db::resolve_pairing(&pool, &new_code).await.unwrap()["device_id"],
        ticket.device.id
    );
    let new_credential = db::random_token();
    db::enroll(
        &pool,
        &ticket.device.id,
        installation,
        &new_code,
        &new_credential,
    )
    .await
    .unwrap();
    assert!(
        db::authenticate_device(&pool, &new_credential)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn rotating_authorization_retires_old_tasks_before_the_same_installation_reenrolls() {
    let (_dir, pool) = database().await;
    let (id, old_credential) = registered(&pool).await;
    let installation: String =
        sqlx::query_scalar("SELECT installation_id FROM devices WHERE device_id=?")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let installation = Uuid::parse_str(&installation).unwrap();
    let ops = manager(&pool);
    let running = ops
        .enqueue("admin", &id, "old-running", Command::ReadConfig {})
        .await
        .unwrap();
    session(&pool, &id, "old-session").await;
    assert_eq!(
        ops.next(&id, "old-session")
            .await
            .unwrap()
            .unwrap()
            .operation
            .operation_id,
        running.operation_id
    );
    let pending_read = ops
        .enqueue("admin", &id, "old-read", Command::ReadDiagnostics {})
        .await
        .unwrap();
    let pending_write = ops
        .enqueue(
            "admin",
            &id,
            "old-restart",
            Command::Restart {
                expected_revision: "a".repeat(64),
                administrator_confirmed: true,
            },
        )
        .await
        .unwrap();

    let code = db::random_authorization_code();
    db::rotate_authorization(&pool, &secrets(), &id, &code, "admin")
        .await
        .unwrap();
    assert!(
        db::authenticate_device(&pool, &old_credential)
            .await
            .is_err()
    );
    for (operation, expected_state, expected_error) in [
        (&running, OperationState::Unknown, "authorization_rotated"),
        (
            &pending_read,
            OperationState::Failed,
            "authorization_rotated",
        ),
        (
            &pending_write,
            OperationState::Failed,
            "authorization_rotated",
        ),
    ] {
        let view = ops
            .get_for_actor("admin", &operation.operation_id)
            .await
            .unwrap();
        assert_eq!(view.state, expected_state);
        let error: String =
            sqlx::query_scalar("SELECT error_code FROM _sarmg_operations WHERE operation_id=?")
                .bind(&operation.operation_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(error, expected_error);
        let events: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM _sarmg_operation_audit_outbox \
             WHERE operation_id=? AND to_state=?",
        )
        .bind(&operation.operation_id)
        .bind(expected_state.as_str())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(events, 1);
    }

    db::enroll(&pool, &id, installation, &code, &db::random_token())
        .await
        .unwrap();
    session(&pool, &id, "new-session").await;
    assert!(ops.next(&id, "new-session").await.unwrap().is_none());
}

#[tokio::test]
async fn deleting_an_instance_removes_it_and_invalidates_its_client_credential() {
    let (_dir, pool) = database().await;
    let ticket = db::create_device(&pool, &secrets(), "delete", "admin")
        .await
        .unwrap();
    let credential = db::random_token();
    db::enroll(
        &pool,
        &ticket.device.id,
        Uuid::new_v4(),
        &ticket.token,
        &credential,
    )
    .await
    .unwrap();

    db::delete_device(&pool, &ticket.device.id, "admin")
        .await
        .unwrap();

    assert!(db::get_device(&pool, &ticket.device.id).await.is_err());
    assert!(db::authenticate_device(&pool, &credential).await.is_err());
    assert!(db::resolve_pairing(&pool, &ticket.token).await.is_err());
    let action: String = sqlx::query_scalar(
        "SELECT action FROM audit_logs WHERE target=? ORDER BY audit_id DESC LIMIT 1",
    )
    .bind(&ticket.device.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(action, "device.delete");
}

#[tokio::test]
async fn enrollment_is_single_use_hashed_bound_and_revocable() {
    let (_dir, pool) = database().await;
    let ticket = db::create_device(&pool, &secrets(), "设备", "admin")
        .await
        .unwrap();
    let installation = Uuid::new_v4();
    let credential = db::random_token();
    assert!(
        db::enroll(
            &pool,
            &ticket.device.id,
            installation,
            &db::random_token(),
            &credential
        )
        .await
        .is_err()
    );
    let binding = db::enroll(
        &pool,
        &ticket.device.id,
        installation,
        &ticket.token,
        &credential,
    )
    .await
    .unwrap();
    assert_eq!(binding.manager_id, ticket.manager_id);
    assert_eq!(binding.installation_id, installation);
    assert!(
        db::enroll(
            &pool,
            &ticket.device.id,
            installation,
            &ticket.token,
            &credential
        )
        .await
        .is_err()
    );
    let stored = db::authenticate_device(&pool, &credential).await.unwrap();
    assert_eq!(
        stored.credential_hash,
        Some(db::token_hash(&credential).to_vec())
    );
    assert_eq!(
        stored.enrollment_hash,
        Some(db::token_hash(&ticket.token).to_vec())
    );
    db::revoke(&pool, &ticket.device.id, "admin").await.unwrap();
    assert!(db::authenticate_device(&pool, &credential).await.is_err());
    assert!(
        db::cancel_pairing(&pool, &ticket.device.id, "admin")
            .await
            .is_err()
    );
    let audit: String =
        sqlx::query_scalar("SELECT COALESCE(group_concat(detail),'') FROM audit_logs")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!audit.contains(&credential) && !audit.contains(&ticket.token));
}

#[tokio::test]
async fn pairing_has_no_expiry_and_duplicate_installation_is_rejected() {
    let (_dir, pool) = database().await;
    let first = db::create_device(&pool, &secrets(), "first", "admin")
        .await
        .unwrap();
    sqlx::query("UPDATE devices SET created_at_micros=0,updated_at_micros=0")
        .execute(&pool)
        .await
        .unwrap();
    let installation = Uuid::new_v4();
    db::enroll(
        &pool,
        &first.device.id,
        installation,
        &first.token,
        &db::random_token(),
    )
    .await
    .unwrap();
    let other = db::create_device(&pool, &secrets(), "second", "admin")
        .await
        .unwrap();
    assert!(
        db::enroll(
            &pool,
            &other.device.id,
            installation,
            &other.token,
            &db::random_token()
        )
        .await
        .is_err()
    );
    assert!(
        db::create_device(&pool, &secrets(), &"长".repeat(33), "admin")
            .await
            .is_err()
    );
    db::cancel_pairing(&pool, &other.device.id, "admin")
        .await
        .unwrap();
    assert!(
        !db::get_device(&pool, &other.device.id)
            .await
            .unwrap()
            .view(db::now_micros().unwrap())
            .pairing_pending
    );
    assert!(
        db::enroll(
            &pool,
            &other.device.id,
            Uuid::new_v4(),
            &other.token,
            &db::random_token()
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn claims_are_serial_session_fenced_and_revocation_stops_new_work() {
    let (_dir, pool) = database().await;
    let (id, _) = registered(&pool).await;
    let ops = manager(&pool);
    let a = ops
        .enqueue("admin", &id, "one", Command::ReadConfig {})
        .await
        .unwrap();
    assert_eq!(a.state, OperationState::Pending);
    assert!(ops.next(&id, "offline").await.is_err());
    let again = ops
        .enqueue("admin", &id, "one", Command::ReadConfig {})
        .await
        .unwrap();
    assert_eq!(a.operation_id, again.operation_id);
    // Idempotency is scoped by actor/device/action; compare different bodies of the same action.
    ops.enqueue(
        "admin",
        &id,
        "restart",
        Command::Restart {
            expected_revision: "a".repeat(64),
            administrator_confirmed: true,
        },
    )
    .await
    .unwrap();
    assert!(
        ops.enqueue(
            "admin",
            &id,
            "restart",
            Command::Restart {
                expected_revision: "b".repeat(64),
                administrator_confirmed: true
            }
        )
        .await
        .is_err()
    );
    ops.enqueue("admin", &id, "two", Command::ReadConfig {})
        .await
        .unwrap();
    session(&pool, &id, "s1").await;
    let claimed = ops.next(&id, "s1").await.unwrap().unwrap();
    assert!(ops.next(&id, "s1").await.unwrap().is_none());
    session(&pool, &id, "s2").await;
    assert!(
        ops.complete(
            &claimed,
            "s1",
            Report::Rejected {
                reason: Rejection::SunshineUnavailable
            }
        )
        .await
        .is_err()
    );
    ops.disconnected(&claimed).await.unwrap();
    assert_eq!(
        ops.get_for_actor("admin", &a.operation_id)
            .await
            .unwrap()
            .state,
        OperationState::Unknown
    );
    let independent_write = ops.next(&id, "s2").await.unwrap().unwrap();
    assert!(matches!(
        ops.task(&independent_write).unwrap().command,
        Command::Restart { .. }
    ));
    ops.disconnected(&independent_write).await.unwrap();
    db::revoke(&pool, &id, "admin").await.unwrap();
    assert!(ops.next(&id, "s2").await.is_err());
    assert!(
        ops.enqueue("admin", &id, "three", Command::ReadConfig {})
            .await
            .is_err()
    );
}

#[tokio::test]
async fn unknown_reconciliation_is_evidence_not_a_second_state_machine() {
    let (_dir, pool) = database().await;
    let (id, _) = registered(&pool).await;
    let ops = manager(&pool);
    let queued = ops
        .enqueue(
            "admin",
            &id,
            "one",
            Command::Restart {
                expected_revision: "a".repeat(64),
                administrator_confirmed: true,
            },
        )
        .await
        .unwrap();
    session(&pool, &id, "s1").await;
    let claimed = ops.next(&id, "s1").await.unwrap().unwrap();
    assert!(
        ops.get_for_actor("other", &queued.operation_id)
            .await
            .is_err()
    );
    ops.disconnected(&claimed).await.unwrap();
    let installation_id = db::get_device(&pool, &id)
        .await
        .unwrap()
        .installation_id
        .and_then(|value| Uuid::parse_str(&value).ok())
        .unwrap();
    let unknown = ops.uncertain(&id, &installation_id).await.unwrap().unwrap();
    ops.complete(
        &unknown,
        "s1",
        Report::Unknown {
            reason: Uncertainty::NoExecutionRecord,
        },
    )
    .await
    .unwrap();
    let view = ops
        .get_for_actor("admin", &queued.operation_id)
        .await
        .unwrap();
    assert_eq!(view.state, OperationState::Unknown);
    assert!(view.reconciliation.is_some());
    assert_eq!(
        ops.task(&claimed).unwrap().binding.device_id.to_string(),
        id
    );
    ops.recover_startup().await.unwrap();
    assert!(
        !db::get_device(&pool, &id)
            .await
            .unwrap()
            .view(db::now_micros().unwrap())
            .client_online
    );
}

#[tokio::test]
async fn stale_restart_authorization_expires_before_delivery() {
    let (_dir, pool) = database().await;
    let (id, _) = registered(&pool).await;
    let ops = manager(&pool);
    let queued = ops
        .enqueue(
            "admin",
            &id,
            "restart-once",
            Command::Restart {
                expected_revision: "a".repeat(64),
                administrator_confirmed: true,
            },
        )
        .await
        .unwrap();
    sqlx::query(
        "UPDATE _sarmg_operations SET created_at_micros=created_at_micros-? WHERE operation_id=?",
    )
    .bind(16_i64 * 60 * 1_000_000)
    .bind(&queued.operation_id)
    .execute(&pool)
    .await
    .unwrap();
    session(&pool, &id, "s1").await;
    assert!(ops.next(&id, "s1").await.unwrap().is_none());
    let state: (String, Option<String>) =
        sqlx::query_as("SELECT state,error_code FROM _sarmg_operations WHERE operation_id=?")
            .bind(&queued.operation_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        state,
        ("failed".into(), Some("execution_deadline_expired".into()))
    );
}

#[tokio::test]
async fn connection_health_and_effectiveness_are_not_conflated() {
    let (_dir, pool) = database().await;
    let (id, _) = registered(&pool).await;
    session(&pool, &id, "s").await;
    let device = db::get_device(&pool, &id).await.unwrap();
    let view = device.view(db::now_micros().unwrap());
    assert!(view.client_online);
    assert_eq!(view.sunshine_reachable, None);
    assert_eq!(view.configuration_state, "unknown");
    let stale = device.view(db::now_micros().unwrap() + 46_000_000);
    assert!(!stale.client_online);
    assert_eq!(stale.sunshine_reachable, None);
}
