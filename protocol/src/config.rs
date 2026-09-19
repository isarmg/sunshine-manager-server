//! Versioned configuration field policy shared by protocol validation and the Manager UI.
use crate::{InvalidTask, SUPPORTED_SUNSHINE_VERSIONS};
use serde::{Deserialize, Serialize};

const ALL_OPERATING_SYSTEMS: &[&str] = &[
    "linux_x86_64",
    "windows_x86_64",
    "macos_x86_64",
    "macos_aarch64",
];
const DESKTOP_NVIDIA_OPERATING_SYSTEMS: &[&str] = &["linux_x86_64", "windows_x86_64"];
const LOG_LEVELS: &[&str] = &["info", "warning", "error", "fatal"];
const SOFTWARE_PRESETS: &[&str] = &[
    "ultrafast",
    "superfast",
    "veryfast",
    "faster",
    "fast",
    "medium",
    "slow",
    "slower",
    "veryslow",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Text,
    Integer,
    Boolean,
    Select,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FieldDefinition {
    pub key: &'static str,
    pub kind: FieldKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_length: Option<usize>,
    #[serde(skip_serializing_if = "slice_is_empty")]
    pub options: &'static [&'static str],
    pub requires_restart: bool,
    pub supported_sunshine_versions: &'static [&'static str],
    pub operating_systems: &'static [&'static str],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prerequisite: Option<&'static str>,
}

const fn slice_is_empty(value: &&[&str]) -> bool {
    value.is_empty()
}

const fn field(
    key: &'static str,
    kind: FieldKind,
    range: Option<(i64, i64)>,
    maximum_length: Option<usize>,
    options: &'static [&'static str],
    operating_systems: &'static [&'static str],
    prerequisite: Option<&'static str>,
) -> FieldDefinition {
    FieldDefinition {
        key,
        kind,
        minimum: match range {
            Some((minimum, _)) => Some(minimum),
            None => None,
        },
        maximum: match range {
            Some((_, maximum)) => Some(maximum),
            None => None,
        },
        maximum_length,
        options,
        requires_restart: true,
        supported_sunshine_versions: SUPPORTED_SUNSHINE_VERSIONS,
        operating_systems,
        prerequisite,
    }
}

pub const FIELD_DEFINITIONS: &[FieldDefinition] = &[
    field(
        "sunshine_name",
        FieldKind::Text,
        None,
        Some(32),
        &[],
        ALL_OPERATING_SYSTEMS,
        None,
    ),
    field(
        "min_log_level",
        FieldKind::Select,
        None,
        None,
        LOG_LEVELS,
        ALL_OPERATING_SYSTEMS,
        None,
    ),
    field(
        "qp",
        FieldKind::Integer,
        Some((0, 51)),
        None,
        &[],
        ALL_OPERATING_SYSTEMS,
        None,
    ),
    field(
        "hevc_mode",
        FieldKind::Integer,
        Some((0, 3)),
        None,
        &[],
        ALL_OPERATING_SYSTEMS,
        None,
    ),
    field(
        "av1_mode",
        FieldKind::Integer,
        Some((0, 3)),
        None,
        &[],
        ALL_OPERATING_SYSTEMS,
        None,
    ),
    field(
        "min_threads",
        FieldKind::Integer,
        Some((1, 64)),
        None,
        &[],
        ALL_OPERATING_SYSTEMS,
        Some("software_encoder"),
    ),
    field(
        "sw_preset",
        FieldKind::Select,
        None,
        None,
        SOFTWARE_PRESETS,
        ALL_OPERATING_SYSTEMS,
        Some("software_encoder"),
    ),
    field(
        "nvenc_preset",
        FieldKind::Integer,
        Some((1, 7)),
        None,
        &[],
        DESKTOP_NVIDIA_OPERATING_SYSTEMS,
        Some("nvidia_encoder"),
    ),
    field(
        "nvenc_vbv_increase",
        FieldKind::Integer,
        Some((0, 400)),
        None,
        &[],
        DESKTOP_NVIDIA_OPERATING_SYSTEMS,
        Some("nvidia_encoder"),
    ),
    field(
        "nvenc_spatial_aq",
        FieldKind::Boolean,
        None,
        None,
        &[],
        DESKTOP_NVIDIA_OPERATING_SYSTEMS,
        Some("nvidia_encoder"),
    ),
    field(
        "nvenc_h264_cavlc",
        FieldKind::Boolean,
        None,
        None,
        &[],
        DESKTOP_NVIDIA_OPERATING_SYSTEMS,
        Some("nvidia_encoder"),
    ),
];

pub fn field_definition(key: &str) -> Option<&'static FieldDefinition> {
    FIELD_DEFINITIONS.iter().find(|field| field.key == key)
}

pub fn contains_field(key: &str) -> bool {
    field_definition(key).is_some()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldValue {
    Boolean(bool),
    Integer(i64),
    Text(String),
}

impl FieldValue {
    pub fn sunshine_text(&self) -> String {
        match self {
            Self::Boolean(value) => value.to_string(),
            Self::Integer(value) => value.to_string(),
            Self::Text(value) => value.clone(),
        }
    }
}

pub fn validate_field(key: &str, value: &FieldValue) -> Result<(), InvalidTask> {
    let definition = field_definition(key).ok_or(InvalidTask)?;
    let valid = match (definition.kind, value) {
        (FieldKind::Text, FieldValue::Text(value)) => {
            !value.trim().is_empty()
                && value == value.trim()
                && definition
                    .maximum_length
                    .is_none_or(|maximum| value.chars().count() <= maximum)
                && !value.chars().any(|character| {
                    character.is_control() || matches!(character, '#' | '=' | '[' | ']')
                })
        }
        (FieldKind::Integer, FieldValue::Integer(value)) => {
            definition.minimum.is_none_or(|minimum| *value >= minimum)
                && definition.maximum.is_none_or(|maximum| *value <= maximum)
        }
        (FieldKind::Boolean, FieldValue::Boolean(_)) => true,
        (FieldKind::Select, FieldValue::Text(value)) => {
            definition.options.contains(&value.as_str())
        }
        _ => false,
    };
    if valid { Ok(()) } else { Err(InvalidTask) }
}
