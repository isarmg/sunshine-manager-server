use chrono::{DateTime, Local, NaiveDate, Utc};
use uuid::Uuid;
use xscs::{
    AppError,
    crypto::SecretBox,
    db,
    operations::{
        HISTORY_PAGE_SIZE, OperationManager, OperationState, OperationView, server_date_bounds,
    },
};
use xscs_protocol::{
    Capabilities, ClientOs, Command, PROTOCOL, Rejection, Report, SUNSHINE_VERSION, Uncertainty,
};

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
async fn collect_history(
    ops: &OperationManager,
    actor: &str,
    device: &str,
    date: Option<&str>,
) -> Vec<OperationView> {
    let mut result = Vec::new();
    let mut cursor = None;
    let mut unique = std::collections::HashSet::new();
    loop {
        let page = ops
            .history_for_actor(actor, device, date, cursor.as_deref())
            .await
            .unwrap();
        assert!(page.operations.len() <= HISTORY_PAGE_SIZE);
        assert_eq!(
            page.previous_cursor.is_some(),
            cursor.is_some() && !page.operations.is_empty()
        );
        for row in &page.operations {
            assert!(
                unique.insert(row.operation_id.clone()),
                "duplicate operation across pages"
            );
        }
        result.extend(page.operations);
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert!(
        result
            .windows(2)
            .all(|rows| (rows[0].created_at_micros, &rows[0].operation_id)
                > (rows[1].created_at_micros, &rows[1].operation_id))
    );
    result
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

#[tokio::test]
async fn operation_history_uses_creation_time_then_id_instead_of_union_order() {
    let (_dir, pool) = database().await;
    let (id, _) = registered(&pool).await;
    let ops = manager(&pool);
    let mut ids = Vec::new();
    for index in 0..3 {
        ids.push(
            ops.enqueue(
                "admin",
                &id,
                &format!("history-{index}"),
                Command::ReadConfig {},
            )
            .await
            .unwrap()
            .operation_id,
        );
    }
    ids.sort();
    let now = db::now_micros().unwrap();
    for (operation_id, created) in [(&ids[0], now - 2_000_000), (&ids[1], now), (&ids[2], now)] {
        sqlx::query(
            "UPDATE _common_operations SET created_at_micros=?,updated_at_micros=? \
             WHERE operation_id=?",
        )
        .bind(created)
        .bind(created)
        .bind(operation_id)
        .execute(&pool)
        .await
        .unwrap();
    }
    let listed: Vec<String> = collect_history(&ops, "admin", &id, None)
        .await
        .into_iter()
        .map(|operation| operation.operation_id)
        .collect();
    assert_eq!(listed, vec![ids[2].clone(), ids[1].clone(), ids[0].clone()]);
}

#[tokio::test]
async fn complete_history_includes_old_uncertain_work_and_global_blocking_count() {
    let (_dir, pool) = database().await;
    let (id, _) = registered(&pool).await;
    let ops = manager(&pool);
    let old_unknown = ops
        .enqueue("admin", &id, "old-unknown", Command::ReadConfig {})
        .await
        .unwrap();
    sqlx::query(
        "UPDATE _common_operations SET state='unknown',created_at_micros=1 WHERE operation_id=?",
    )
    .bind(&old_unknown.operation_id)
    .execute(&pool)
    .await
    .unwrap();
    let mut newer = Vec::new();
    for index in 0..175 {
        newer.push(
            ops.enqueue(
                "admin",
                &id,
                &format!("bulk-{index}"),
                Command::ReadDiagnostics {},
            )
            .await
            .unwrap()
            .operation_id,
        );
    }
    let other = ops
        .enqueue("second-admin", &id, "other-actor", Command::ReadConfig {})
        .await
        .unwrap();
    let first = ops
        .history_for_actor("admin", &id, None, None)
        .await
        .unwrap();
    assert_eq!(first.operations.len(), HISTORY_PAGE_SIZE);
    assert!(first.previous_cursor.is_none());
    let next = first.next_cursor.as_deref().unwrap();
    let second = ops
        .history_for_actor("admin", &id, None, Some(next))
        .await
        .unwrap();
    let previous = second.previous_cursor.as_deref().unwrap();
    let returned = ops
        .history_for_actor("admin", &id, None, Some(previous))
        .await
        .unwrap();
    assert_eq!(
        returned
            .operations
            .iter()
            .map(|row| &row.operation_id)
            .collect::<Vec<_>>(),
        first
            .operations
            .iter()
            .map(|row| &row.operation_id)
            .collect::<Vec<_>>()
    );
    assert!(returned.previous_cursor.is_none());
    for (actor, device, date, cursor) in [
        ("second-admin", id.as_str(), None, next.to_owned()),
        ("admin", "different-device", None, next.to_owned()),
        ("admin", id.as_str(), Some("2027-01-15"), next.to_owned()),
        ("admin", id.as_str(), None, "!".into()),
        ("admin", id.as_str(), None, "A".repeat(513)),
    ] {
        assert!(matches!(
            ops.history_for_actor(actor, device, date, Some(&cursor))
                .await,
            Err(AppError::BadRequest(_))
        ));
    }
    let listed = collect_history(&ops, "admin", &id, None).await;
    assert_eq!(listed.len(), 176);
    assert_eq!(
        listed.last().unwrap().operation_id,
        old_unknown.operation_id
    );
    assert_eq!(listed.last().unwrap().state, OperationState::Unknown);
    assert!(
        !listed
            .iter()
            .any(|item| item.operation_id == other.operation_id)
    );
    assert_eq!(
        collect_history(&ops, "second-admin", &id, None).await.len(),
        1
    );
    assert_eq!(ops.summary_for_device(&id).await.unwrap().blocking_count, 0);
    let recent = ops.recent_for_actor("admin", &id).await.unwrap();
    assert_eq!(recent.len(), 50);
    assert!(
        !recent
            .iter()
            .any(|item| item.operation_id == old_unknown.operation_id)
    );
    assert!(
        !recent
            .iter()
            .any(|item| item.operation_id == other.operation_id)
    );

    // Same-time records have a stable ID tie-breaker; a new insert must not hide old work.
    let tie_time = db::now_micros().unwrap() + 1_000_000;
    for operation_id in [&newer[0], &newer[1]] {
        sqlx::query(
            "UPDATE _common_operations SET created_at_micros=?,updated_at_micros=? \
             WHERE operation_id=?",
        )
        .bind(tie_time)
        .bind(tie_time)
        .bind(operation_id)
        .execute(&pool)
        .await
        .unwrap();
    }
    let after = collect_history(&ops, "admin", &id, None).await;
    let tied = after
        .iter()
        .filter(|item| item.created_at_micros == tie_time)
        .map(|item| item.operation_id.as_str())
        .collect::<Vec<_>>();
    let mut expected = vec![newer[0].as_str(), newer[1].as_str()];
    expected.sort_by(|a, b| b.cmp(a));
    assert_eq!(tied, expected);
    let current_first = ops
        .history_for_actor("admin", &id, None, None)
        .await
        .unwrap();
    let next = current_first.next_cursor.as_deref().unwrap();
    let second = ops
        .history_for_actor("admin", &id, None, Some(next))
        .await
        .unwrap();
    let inserted = ops
        .enqueue("admin", &id, "concurrent-insert", Command::ReadConfig {})
        .await
        .unwrap();
    let anchored_second = ops
        .history_for_actor("admin", &id, None, Some(next))
        .await
        .unwrap();
    assert_eq!(
        anchored_second
            .operations
            .iter()
            .map(|row| &row.operation_id)
            .collect::<Vec<_>>(),
        second
            .operations
            .iter()
            .map(|row| &row.operation_id)
            .collect::<Vec<_>>()
    );
    let after_insert = collect_history(&ops, "admin", &id, None).await;
    assert_eq!(after_insert.len(), 177);
    assert!(
        after_insert
            .iter()
            .any(|item| item.operation_id == inserted.operation_id)
    );
    assert!(
        after_insert
            .iter()
            .any(|item| item.operation_id == old_unknown.operation_id)
    );

    // A selected day includes both its first microsecond and all older uncertain work,
    // excludes the next midnight, and never reveals another administrator's details.
    let (from, to) = server_date_bounds("2027-01-15").unwrap();
    sqlx::query(
        "UPDATE _common_operations SET created_at_micros=?,updated_at_micros=? WHERE target_key=?",
    )
    .bind(from + 1)
    .bind(to)
    .bind(&id)
    .execute(&pool)
    .await
    .unwrap();
    for (operation_id, boundary) in [
        (&old_unknown.operation_id, from),
        (&newer[0], from - 1),
        (&inserted.operation_id, to),
    ] {
        sqlx::query("UPDATE _common_operations SET created_at_micros=? WHERE operation_id=?")
            .bind(boundary)
            .bind(operation_id)
            .execute(&pool)
            .await
            .unwrap();
    }
    let dated = collect_history(&ops, "admin", &id, Some("2027-01-15")).await;
    assert_eq!(dated.len(), 175);
    assert_eq!(dated.last().unwrap().operation_id, old_unknown.operation_id);
    assert!(
        dated
            .iter()
            .all(|item| item.created_at_server.starts_with("2027-01-15 "))
    );
    assert!(
        !dated
            .iter()
            .any(|item| item.operation_id == inserted.operation_id
                || item.operation_id == newer[0]
                || item.operation_id == other.operation_id)
    );
    let mut ranged = Vec::new();
    let mut cursor = None;
    loop {
        let page = ops
            .history_range_for_actor("admin", &id, "2027-01-14", "2027-01-15", cursor.as_deref())
            .await
            .unwrap();
        assert!(page.operations.len() <= HISTORY_PAGE_SIZE);
        if let Some(next) = &page.next_cursor {
            assert!(
                ops.history_range_for_actor("admin", &id, "2027-01-14", "2027-01-16", Some(next))
                    .await
                    .is_err()
            );
        }
        ranged.extend(page.operations);
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(ranged.len(), 176);
    assert!(ranged.iter().any(|row| row.operation_id == newer[0]));
    assert!(
        !ranged
            .iter()
            .any(|row| row.operation_id == inserted.operation_id
                || row.operation_id == other.operation_id)
    );
    assert!(matches!(
        ops.history_for_actor("admin", &id, Some("2027-02-30"), None)
            .await,
        Err(AppError::BadRequest(_))
    ));
}

#[test]
fn server_calendar_uses_local_day_boundaries() {
    for date in ["2027-01-15", "2027-03-14", "2027-11-07"] {
        let day = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
        let (from, to) = server_date_bounds(date).unwrap();
        assert!(from < to);
        let at_start = DateTime::<Utc>::from_timestamp_micros(from)
            .unwrap()
            .with_timezone(&Local);
        let before_end = DateTime::<Utc>::from_timestamp_micros(to - 1)
            .unwrap()
            .with_timezone(&Local);
        let at_end = DateTime::<Utc>::from_timestamp_micros(to)
            .unwrap()
            .with_timezone(&Local);
        assert_eq!(at_start.date_naive(), day);
        assert_eq!(before_end.date_naive(), day);
        assert_eq!(at_end.date_naive(), day.succ_opt().unwrap());
    }
    if std::env::var("TZ").as_deref() == Ok("America/New_York") {
        let (spring_start, spring_end) = server_date_bounds("2027-03-14").unwrap();
        let (fall_start, fall_end) = server_date_bounds("2027-11-07").unwrap();
        assert_eq!(spring_end - spring_start, 23 * 60 * 60 * 1_000_000);
        assert_eq!(fall_end - fall_start, 25 * 60 * 60 * 1_000_000);
    }
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
        managed_fields: xscs_protocol::config::FIELD_DEFINITIONS
            .iter()
            .map(|field| field.key.to_owned())
            .collect(),
        configuration_overwrite: true,
        application_management: true,
        application_host_commands_allowed: true,
        moonlight_pairing_management: true,
        pending_pairing_listing: true,
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
        assert_eq!(view.status_reason, Some("authorization_rotated"));
        let api_value = serde_json::to_value(&view).unwrap();
        assert_eq!(api_value["status_reason"], "authorization_rotated");
        assert!(api_value.get("error_code").is_none());
        let error: String =
            sqlx::query_scalar("SELECT error_code FROM _common_operations WHERE operation_id=?")
                .bind(&operation.operation_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(error, expected_error);
        let events: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM _common_operation_audit_outbox \
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
            .unwrap()
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
        OperationState::Failed
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
            .unwrap()
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
        "UPDATE _common_operations SET created_at_micros=created_at_micros-? WHERE operation_id=?",
    )
    .bind(16_i64 * 60 * 1_000_000)
    .bind(&queued.operation_id)
    .execute(&pool)
    .await
    .unwrap();
    session(&pool, &id, "s1").await;
    assert!(ops.next(&id, "s1").await.unwrap().is_none());
    let state: (String, Option<String>) =
        sqlx::query_as("SELECT state,error_code FROM _common_operations WHERE operation_id=?")
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
    let view = device.view(db::now_micros().unwrap()).unwrap();
    assert!(view.client_online);
    assert_eq!(view.sunshine_reachable, None);
    assert_eq!(view.configuration_state, "unknown");
    let stale = device.view(db::now_micros().unwrap() + 46_000_000).unwrap();
    assert!(!stale.client_online);
    assert_eq!(stale.sunshine_reachable, None);
}

#[tokio::test]
async fn invalid_persisted_observations_fail_without_becoming_missing_data() {
    let (_dir, pool) = database().await;
    let (id, _) = registered(&pool).await;
    for column in ["capabilities_json", "snapshot_json"] {
        let mut device = db::get_device(&pool, &id).await.unwrap();
        if column == "capabilities_json" {
            device.capabilities_json = Some("{}".into());
        } else {
            device.snapshot_json = Some("{}".into());
        }
        assert!(device.view(db::now_micros().unwrap()).is_err());
    }
    sqlx::query("UPDATE devices SET capabilities_json=json_remove(capabilities_json,'$.configuration_overwrite') WHERE device_id=?")
        .bind(&id).execute(&pool).await.unwrap();
    let before: String =
        sqlx::query_scalar("SELECT capabilities_json FROM devices WHERE device_id=?")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(db::list_devices(&pool).await.is_err());
    assert!(
        db::require_current_runtime_state(&pool, &secrets())
            .await
            .is_err()
    );
    let after: String =
        sqlx::query_scalar("SELECT capabilities_json FROM devices WHERE device_id=?")
            .bind(&id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn disconnected_read_does_not_block_and_replayed_results_are_frozen() {
    let (_dir, pool) = database().await;
    let (id, _) = registered(&pool).await;
    let ops = manager(&pool);
    let queued = ops
        .enqueue("admin", &id, "read-disconnect", Command::ReadConfig {})
        .await
        .unwrap();
    session(&pool, &id, "s1").await;
    let claim = ops.next(&id, "s1").await.unwrap().unwrap();
    ops.disconnected(&claim).await.unwrap();
    assert_eq!(ops.summary_for_device(&id).await.unwrap().blocking_count, 0);
    assert_eq!(
        ops.get_for_actor("admin", &queued.operation_id)
            .await
            .unwrap()
            .state,
        OperationState::Failed
    );
    let queued = ops
        .enqueue("admin", &id, "read-next", Command::ReadConfig {})
        .await
        .unwrap();
    let claim = ops.next(&id, "s1").await.unwrap().unwrap();
    let report = Report::Rejected {
        reason: Rejection::SunshineUnavailable,
    };
    ops.complete(&claim, "s1", report.clone()).await.unwrap();
    let binding = ops.task(&claim).unwrap().binding;
    let finished = ops
        .receipt_operation(&queued.operation_id, &binding)
        .await
        .unwrap();
    ops.accept_replayed_result(&finished, "s1", report)
        .await
        .unwrap();
    assert!(
        ops.accept_replayed_result(
            &finished,
            "s1",
            Report::Rejected {
                reason: Rejection::InvalidTask
            }
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn config_overwrite_is_dispatched_without_a_revision_guard_and_updates_the_snapshot() {
    let (_dir, pool) = database().await;
    let (id, _) = registered(&pool).await;
    let ops = manager(&pool);
    let set = std::collections::BTreeMap::from([(
        "qp".into(),
        xscs_protocol::config::FieldValue::Integer(29),
    )]);
    let remove = xscs_protocol::config::FIELD_DEFINITIONS
        .iter()
        .filter(|field| {
            field.operating_systems.contains(&"linux_x86_64") && !set.contains_key(field.key)
        })
        .map(|field| field.key.to_owned())
        .collect();
    let queued = ops
        .enqueue(
            "admin",
            &id,
            "overwrite",
            Command::SaveConfig {
                set,
                remove,
                restart_policy: xscs_protocol::RestartPolicy::Manual,
            },
        )
        .await
        .unwrap();
    assert_eq!(queued.action, "sunshine.config.save");
    session(&pool, &id, "overwrite-session").await;
    let claim = ops.next(&id, "overwrite-session").await.unwrap().unwrap();
    assert!(matches!(
        ops.task(&claim).unwrap().command,
        Command::SaveConfig { .. }
    ));
    ops.complete(
        &claim,
        "overwrite-session",
        Report::ConfigSaved {
            snapshot: xscs_protocol::ConfigSnapshot {
                revision: "a".repeat(64),
                sunshine_version: SUNSHINE_VERSION.into(),
                fields: std::collections::BTreeMap::from([("qp".into(), "29".into())]),
                effectiveness: xscs_protocol::Effectiveness::AwaitingRestart,
            },
        },
    )
    .await
    .unwrap();
    let device = db::get_device(&pool, &id).await.unwrap();
    assert_eq!(
        device
            .view(db::now_micros().unwrap())
            .unwrap()
            .configuration_state,
        "awaiting_restart"
    );
}

#[tokio::test]
async fn explicitly_unavailable_controls_are_rejected_independently_of_software_version() {
    for software_version in ["0.3.0", "0.5.1"] {
        let (_dir, pool) = database().await;
        let (id, _) = registered(&pool).await;
        sqlx::query("UPDATE devices SET capabilities_json=json_set(capabilities_json,'$.client_version',?,'$.configuration_overwrite',json('false'),'$.pending_pairing_listing',json('false')) WHERE device_id=?")
        .bind(software_version).bind(&id).execute(&pool).await.unwrap();
        let ops = manager(&pool);
        let remove = xscs_protocol::config::FIELD_DEFINITIONS
            .iter()
            .filter(|field| field.operating_systems.contains(&"linux_x86_64"))
            .map(|field| field.key.to_owned())
            .collect();
        for (key, command) in [
            (
                "unavailable-overwrite",
                Command::SaveConfig {
                    set: Default::default(),
                    remove,
                    restart_policy: xscs_protocol::RestartPolicy::Manual,
                },
            ),
            ("unavailable-pairings", Command::ListPendingPairings {}),
        ] {
            assert!(matches!(
                ops.enqueue("admin", &id, key, command).await,
                Err(AppError::BadRequest(_))
            ));
        }
        assert!(collect_history(&ops, "admin", &id, None).await.is_empty());
        assert!(
            ops.enqueue("admin", &id, "available-read", Command::ReadConfig {})
                .await
                .is_ok()
        );
    }
}

#[tokio::test]
async fn software_release_labels_do_not_disable_explicitly_reported_current_controls() {
    let (_dir, pool) = database().await;
    let (id, _) = registered(&pool).await;
    sqlx::query("UPDATE devices SET capabilities_json=json_set(capabilities_json,'$.client_version','0.3.0') WHERE device_id=?")
        .bind(&id).execute(&pool).await.unwrap();
    let ops = manager(&pool);
    let remove = xscs_protocol::config::FIELD_DEFINITIONS
        .iter()
        .filter(|field| field.operating_systems.contains(&"linux_x86_64"))
        .map(|field| field.key.to_owned())
        .collect();
    let overwrite = ops
        .enqueue(
            "admin",
            &id,
            "reported-overwrite",
            Command::SaveConfig {
                set: Default::default(),
                remove,
                restart_policy: xscs_protocol::RestartPolicy::Manual,
            },
        )
        .await
        .unwrap();
    let pairings = ops
        .enqueue(
            "admin",
            &id,
            "reported-pairings",
            Command::ListPendingPairings {},
        )
        .await
        .unwrap();
    assert_eq!(overwrite.action, "sunshine.config.save");
    assert_eq!(pairings.action, "sunshine.pairing.pending.list");
    assert_eq!(collect_history(&ops, "admin", &id, None).await.len(), 2);
}

#[tokio::test]
async fn unresolved_write_allows_diagnostics_and_preserves_idempotency() {
    let (_dir, pool) = database().await;
    let (id, _) = registered(&pool).await;
    let ops = manager(&pool);
    let command = Command::Restart {
        expected_revision: "a".repeat(64),
        administrator_confirmed: true,
    };
    let queued = ops
        .enqueue("admin", &id, "restart-unknown", command.clone())
        .await
        .unwrap();
    session(&pool, &id, "s1").await;
    let claim = ops.next(&id, "s1").await.unwrap().unwrap();
    ops.disconnected(&claim).await.unwrap();
    assert_eq!(
        ops.enqueue("admin", &id, "restart-unknown", command.clone())
            .await
            .unwrap()
            .operation_id,
        queued.operation_id
    );
    assert!(
        ops.enqueue("admin", &id, "second-restart", command)
            .await
            .is_err()
    );
    ops.enqueue("admin", &id, "diagnostic", Command::ReadDiagnostics {})
        .await
        .unwrap();
    assert!(matches!(
        ops.task(&ops.next(&id, "s1").await.unwrap().unwrap())
            .unwrap()
            .command,
        Command::ReadDiagnostics {}
    ));
    assert_eq!(ops.summary_for_device(&id).await.unwrap().blocking_count, 1);
}

#[tokio::test]
async fn oldest_write_is_not_starved_by_newer_reads() {
    let (_dir, pool) = database().await;
    let (id, _) = registered(&pool).await;
    let ops = manager(&pool);
    let write = ops
        .enqueue(
            "admin",
            &id,
            "first-write",
            Command::Restart {
                expected_revision: "a".repeat(64),
                administrator_confirmed: true,
            },
        )
        .await
        .unwrap();
    for index in 0..20 {
        ops.enqueue(
            "admin",
            &id,
            &format!("newer-read-{index}"),
            Command::ReadConfig {},
        )
        .await
        .unwrap();
    }
    session(&pool, &id, "s1").await;
    assert_eq!(
        ops.next(&id, "s1")
            .await
            .unwrap()
            .unwrap()
            .operation
            .operation_id,
        write.operation_id
    );
}

#[tokio::test]
async fn oversized_stored_report_is_rejected_without_fetching_its_full_blob() {
    let (_directory, pool) = database().await;
    let (device, _) = registered(&pool).await;
    let ops = manager(&pool);
    let row = ops
        .enqueue("admin", &device, "large-result", Command::ReadConfig {})
        .await
        .unwrap();
    sqlx::query("UPDATE _common_operations SET result_payload=zeroblob(?) WHERE operation_id=?")
        .bind((xscs_protocol::MAX_REPORT_BYTES * 4) as i64)
        .bind(row.operation_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        ops.history_for_actor("admin", &device, None, None).await,
        Err(AppError::Internal(_))
    ));
    // An unrelated instance still has a working management path and a separate history.
    let (other, _) = registered(&pool).await;
    assert!(
        ops.history_for_actor("admin", &other, None, None)
            .await
            .unwrap()
            .operations
            .is_empty()
    );
    assert!(
        ops.enqueue("admin", &other, "other-instance", Command::ReadConfig {})
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn oversized_stored_history_text_fails_closed_without_blocking_other_instances() {
    let (_directory, pool) = database().await;
    let (device, _) = registered(&pool).await;
    let (other, _) = registered(&pool).await;
    let ops = manager(&pool);
    let row = ops
        .enqueue("admin", &device, "large-text", Command::ReadConfig {})
        .await
        .unwrap();
    let oversized = "x".repeat(1024 * 1024);
    for column in ["action", "error_code", "resolution_code"] {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "UPDATE _common_operations SET {column}=? WHERE operation_id=?"
        )))
        .bind(&oversized)
        .bind(&row.operation_id)
        .execute(&pool)
        .await
        .unwrap();
        assert!(matches!(
            ops.history_for_actor("admin", &device, None, None).await,
            Err(AppError::Internal(_))
        ));
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "UPDATE _common_operations SET {column}=? WHERE operation_id=?"
        )))
        .bind(if column == "action" {
            Some(row.action.as_str())
        } else {
            None
        })
        .bind(&row.operation_id)
        .execute(&pool)
        .await
        .unwrap();
    }
    assert_eq!(
        ops.history_for_actor("admin", &device, None, None)
            .await
            .unwrap()
            .operations
            .len(),
        1
    );
    assert!(
        ops.enqueue("admin", &other, "other-instance", Command::ReadConfig {})
            .await
            .is_ok()
    );
    assert_eq!(
        ops.history_for_actor("admin", &other, None, None)
            .await
            .unwrap()
            .operations
            .len(),
        1
    );
}
