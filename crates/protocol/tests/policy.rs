use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;
use xscs_protocol::{config::*, *};

fn task(command: Command, permission: Permission) -> Task {
    Task {
        protocol: TASK_PROTOCOL.into(),
        operation_id: "op_00000000-0000-4000-8000-000000000001".into(),
        binding: Binding {
            manager_id: Uuid::from_u128(1),
            device_id: Uuid::from_u128(2),
            installation_id: Uuid::from_u128(3),
        },
        permission,
        command,
    }
}

fn capabilities() -> Capabilities {
    Capabilities {
        protocol: PROTOCOL.into(),
        client_version: "0.2.0".into(),
        os: ClientOs::LinuxX86_64,
        sunshine_version: SUNSHINE_VERSION.into(),
        restart_allowed: true,
        managed_fields: FIELD_DEFINITIONS
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
    }
}

#[test]
fn moonlight_pairing_uses_bounded_pending_requests_and_validates_pin() {
    let caps = capabilities();
    let pairing = PendingPairing {
        id: "0123456789abcdef0123456789abcdef".into(),
        name: "Moonlight TV".into(),
        address: "192.168.1.20".into(),
    };
    let read = Command::ListPendingPairings {};
    let read_task = task(read.clone(), Permission::ManagePairing);
    let mut unavailable = caps.clone();
    unavailable.pending_pairing_listing = false;
    assert_eq!(
        read_task.validate(&read_task.binding, &unavailable),
        Err(Rejection::UnsupportedCapability)
    );
    assert!(read_task.validate(&read_task.binding, &caps).is_ok());
    assert!(
        validate_report(
            &read,
            &Report::PendingPairingsRead {
                pairings: vec![pairing.clone()],
            }
        )
        .is_ok()
    );
    assert!(validate_pending_pairings(&vec![pairing.clone(); 2]).is_err());
    assert!(validate_pending_pairings(&vec![pairing.clone(); 65]).is_err());
    let mut malformed = pairing.clone();
    malformed.id = "manual-id".into();
    assert!(validate_pending_pairings(&[malformed]).is_err());
    let mut unsafe_name = pairing.clone();
    unsafe_name.name = "TV\nlocalhost".into();
    assert!(validate_pending_pairings(&[unsafe_name]).is_err());
    for pin in ["12a4", "123", "12345"] {
        let submit = task(
            Command::SubmitPairingPin {
                pairing_id: pairing.id.clone(),
                pin: pin.into(),
                name: pairing.name.clone(),
            },
            Permission::ManagePairing,
        );
        assert_eq!(
            submit.validate(&submit.binding, &caps),
            Err(Rejection::InvalidTask)
        );
    }
    let submit = task(
        Command::SubmitPairingPin {
            pairing_id: pairing.id,
            pin: "1234".into(),
            name: pairing.name,
        },
        Permission::ManagePairing,
    );
    assert!(submit.validate(&submit.binding, &caps).is_ok());
    assert!(
        validate_report(
            &submit.command,
            &Report::Rejected {
                reason: Rejection::PairingFailed
            }
        )
        .is_ok()
    );
}

#[test]
fn closed_protocol_rejects_arbitrary_execution_and_unknown_fields() {
    for bytes in [
        br#"{"type":"http","url":"https://localhost/"}"#.as_slice(),
        br#"{"type":"revoked","script":"evil"}"#,
    ] {
        assert!(decode_manager_message(bytes).is_err());
    }
    assert!(decode_manager_message(&vec![b' '; MAX_MESSAGE_BYTES + 1]).is_err());
}

#[test]
fn credentials_commands_network_and_paths_are_never_managed_fields() {
    for key in [
        "global_prep_cmd",
        "prep-cmd",
        "cmd",
        "file_apps",
        "pkey",
        "credentials_file",
        "upnp",
        "port",
        "csrf_allowed_origins",
        "origin_web_ui_allowed",
        "install_steam_audio_drivers",
    ] {
        assert!(
            validate_field(key, &FieldValue::Text("anything".into())).is_err(),
            "{key}"
        );
        let task = task(
            Command::PatchConfig {
                expected_revision: "a".repeat(64),
                set: BTreeMap::new(),
                remove: BTreeSet::from([key.into()]),
                restart_policy: RestartPolicy::Manual,
            },
            Permission::WriteConfig,
        );
        assert!(
            task.validate(&task.binding, &capabilities()).is_err(),
            "{key}"
        );
    }
}

#[test]
fn field_definitions_are_unique_and_drive_validation_metadata() {
    let definitions = xscs_protocol::config::FIELD_DEFINITIONS;
    let keys = definitions
        .iter()
        .map(|field| field.key)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(keys.len(), definitions.len());
    for field in definitions {
        assert!(field.requires_restart);
        assert_eq!(
            field.supported_sunshine_versions,
            xscs_protocol::SUPPORTED_SUNSHINE_VERSIONS
        );
        assert!(!field.operating_systems.is_empty());
    }
    let json = serde_json::to_value(definitions).unwrap();
    assert_eq!(json.as_array().unwrap().len(), definitions.len());
    assert_eq!(
        json.as_array().unwrap()[0]["maximum_length"],
        serde_json::json!(32)
    );
}

#[test]
fn field_types_ranges_and_config_line_injection_are_rejected() {
    for value in [
        FieldValue::Integer(-1),
        FieldValue::Integer(52),
        FieldValue::Text("28".into()),
        FieldValue::Boolean(true),
    ] {
        assert!(validate_field("qp", &value).is_err());
    }
    for value in [
        "host\nupnp = enabled".into(),
        "a".repeat(33),
        "x#y".into(),
        "x\rname".into(),
        "".into(),
    ] {
        assert!(validate_field("sunshine_name", &FieldValue::Text(value)).is_err());
    }
    assert!(validate_field("sunshine_name", &FieldValue::Text("猫".repeat(32))).is_ok());
    assert!(validate_field("sunshine_name", &FieldValue::Text("🎮".repeat(32))).is_ok());
    assert!(validate_field("min_log_level", &FieldValue::Text("debug".into())).is_ok());
    assert!(validate_field("qp", &FieldValue::Integer(28)).is_ok());
}

#[test]
fn binding_protocol_and_permissions_are_enforced() {
    let mut task = task(Command::ReadConfig {}, Permission::ReadConfig);
    assert!(task.validate(&task.binding, &capabilities()).is_ok());
    let mut wrong = task.binding.clone();
    wrong.device_id = Uuid::from_u128(4);
    assert_eq!(
        task.validate(&wrong, &capabilities()),
        Err(Rejection::BindingMismatch)
    );
    task.permission = Permission::WriteConfig;
    assert_eq!(
        task.validate(&task.binding, &capabilities()),
        Err(Rejection::PermissionDenied)
    );
    task.protocol = "sunshine-management/1".into();
    assert_eq!(
        task.validate(&task.binding, &capabilities()),
        Err(Rejection::InvalidTask)
    );
}

#[test]
fn restart_requires_reported_capability_and_administrator_confirmation() {
    let mut task = task(
        Command::Restart {
            expected_revision: "f".repeat(64),
            administrator_confirmed: true,
        },
        Permission::Restart,
    );
    assert!(task.validate(&task.binding, &capabilities()).is_ok());
    let mut no_restart = capabilities();
    no_restart.restart_allowed = false;
    assert_eq!(
        task.validate(&task.binding, &no_restart),
        Err(Rejection::PermissionDenied)
    );
    task.command = Command::Restart {
        expected_revision: "f".repeat(64),
        administrator_confirmed: false,
    };
    assert_eq!(
        task.validate(&task.binding, &capabilities()),
        Err(Rejection::PermissionDenied)
    );
}

#[test]
fn fingerprint_covers_ownership_permissions_and_content() {
    let mut task = task(Command::ReadConfig {}, Permission::ReadConfig);
    let original = task.fingerprint().unwrap();
    task.binding.installation_id = Uuid::from_u128(4);
    assert_ne!(original, task.fingerprint().unwrap());
    let before = task.fingerprint().unwrap();
    task.permission = Permission::Restart;
    assert_ne!(before, task.fingerprint().unwrap());
}

#[test]
fn session_capabilities_do_not_change_durable_task_fingerprints() {
    let task = task(Command::ReadConfig {}, Permission::ReadConfig);
    assert_ne!(PROTOCOL, TASK_PROTOCOL);
    assert_eq!(
        task.fingerprint().unwrap(),
        "a2868c8102cf8023023c2d2857babd0dc4518aab607b28a93f3d1a92b0199e78"
    );
    let mut incompatible = capabilities();
    incompatible.protocol = TASK_PROTOCOL.into();
    assert_eq!(
        task.validate(&task.binding, &incompatible),
        Err(Rejection::InvalidTask)
    );
}
