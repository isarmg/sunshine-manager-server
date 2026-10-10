use std::{
    collections::BTreeMap,
    env, fs,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use anyhow::Context;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use xcss::config::{ConfigSource, EnvMapping, EnvValueKind, Override};

use crate::crypto::SecretBox;

#[derive(Clone)]
pub struct ServeConfig {
    pub bind: SocketAddr,
    pub database_url: String,
    pub production: bool,
    pub secrets: SecretBox,
    pub bootstrap_admin_username: String,
    pub bootstrap_admin_password: Option<String>,
    pub static_dir: Option<PathBuf>,
    pub data_dir: PathBuf,
    pub sources: BTreeMap<String, ConfigSource>,
}

impl ServeConfig {
    pub fn from_runtime() -> anyhow::Result<Self> {
        Self::from_sources(None, None, None)
    }

    pub fn from_sources(
        config: Option<&Path>,
        data_dir: Option<&Path>,
        bind: Option<SocketAddr>,
    ) -> anyhow::Result<Self> {
        let config = config.map(normalize_config_path).transpose()?;
        let file = config
            .as_deref()
            .map(xcss::config::read_private_file)
            .transpose()?;
        let environment = xcss::config::read_environment(&ENVIRONMENT, |name| env::var(name).ok())?;
        let mut command_line = Vec::new();
        if let Some(path) = data_dir {
            command_line.push(Override::new(
                "/data_dir",
                path.to_string_lossy().into_owned(),
            ));
        }
        if let Some(bind) = bind {
            command_line.push(Override::new("/bind", bind.to_string()));
        }
        let loaded = xcss::config::resolve_validated(
            &Settings::default(),
            file.as_deref(),
            &environment,
            &command_line,
            validate_intrinsic,
        )?;
        let settings = loaded.value;
        let database_url = match settings.database_url {
            Some(url) => url,
            None => format!(
                "sqlite://{}",
                settings
                    .data_dir
                    .as_ref()
                    .context("data_dir or database_url is required")?
                    .join("xscs.sqlite3")
                    .display()
            ),
        };
        let database_path = crate::database_schema::database_path(&database_url)?;
        let data_dir = settings.data_dir.unwrap_or_else(|| {
            database_path
                .parent()
                .expect("database has a parent")
                .to_path_buf()
        });
        anyhow::ensure!(
            data_dir.is_absolute() && database_path.parent() == Some(data_dir.as_path()),
            "database must be a direct child of the absolute data_dir"
        );
        let credential_key = decode_key(
            settings
                .credential_key
                .as_deref()
                .context("credential_key is required")?,
        )?;
        let bind: SocketAddr = settings
            .bind
            .parse()
            .context("bind must be a socket address")?;
        let production = settings.production;
        let static_dir =
            development_static_dir(settings.development_web_dir.as_deref(), !production)?;
        if !bind.ip().is_loopback() {
            anyhow::bail!("Manager must bind loopback behind a trusted HTTPS/WSS ingress");
        }

        let bootstrap_admin_username =
            xcss::admin_auth::normalize_administrator_username(&settings.bootstrap_admin_username)?;

        Ok(Self {
            bind,
            database_url,
            production,
            secrets: SecretBox::new(settings.credential_key_id, credential_key)?,
            bootstrap_admin_username,
            bootstrap_admin_password: settings.bootstrap_admin_password,
            static_dir,
            data_dir,
            sources: loaded.sources,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    database_url: Option<String>,
    data_dir: Option<PathBuf>,
    bind: String,
    production: bool,
    credential_key: Option<String>,
    credential_key_id: String,
    bootstrap_admin_username: String,
    bootstrap_admin_password: Option<String>,
    development_web_dir: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            database_url: None,
            data_dir: None,
            bind: "127.0.0.1:18104".into(),
            production: true,
            credential_key: None,
            credential_key_id: "primary".into(),
            bootstrap_admin_username: "admin".into(),
            bootstrap_admin_password: None,
            development_web_dir: None,
        }
    }
}

const ENVIRONMENT: [EnvMapping<'static>; 9] = [
    EnvMapping {
        variable: "XSCS_DATABASE_URL",
        path: "/database_url",
        kind: EnvValueKind::String,
    },
    EnvMapping {
        variable: "XSCS_DATA_DIR",
        path: "/data_dir",
        kind: EnvValueKind::String,
    },
    EnvMapping {
        variable: "XSCS_BIND",
        path: "/bind",
        kind: EnvValueKind::String,
    },
    EnvMapping {
        variable: "XSCS_PRODUCTION",
        path: "/production",
        kind: EnvValueKind::Boolean,
    },
    EnvMapping {
        variable: "XSCS_CREDENTIAL_KEY",
        path: "/credential_key",
        kind: EnvValueKind::String,
    },
    EnvMapping {
        variable: "XSCS_CREDENTIAL_KEY_ID",
        path: "/credential_key_id",
        kind: EnvValueKind::String,
    },
    EnvMapping {
        variable: "XSCS_BOOTSTRAP_ADMIN_USERNAME",
        path: "/bootstrap_admin_username",
        kind: EnvValueKind::String,
    },
    EnvMapping {
        variable: "XSCS_BOOTSTRAP_ADMIN_PASSWORD",
        path: "/bootstrap_admin_password",
        kind: EnvValueKind::String,
    },
    EnvMapping {
        variable: "XCSS_DEV_WEB_DIR",
        path: "/development_web_dir",
        kind: EnvValueKind::String,
    },
];

fn development_static_dir(
    value: Option<&str>,
    development: bool,
) -> anyhow::Result<Option<PathBuf>> {
    let Some(value) = value else { return Ok(None) };
    anyhow::ensure!(
        development && crate::release_contract::SOURCE_REVISION == "unbound",
        "directory Web assets are allowed only in an unbound development build"
    );
    Ok(Some(validate_static_dir(value, false)?))
}

fn validate_static_dir(value: &str, production: bool) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(
        !production,
        "production Web assets are embedded in the binary"
    );
    let root = Path::new(value);
    anyhow::ensure!(
        root.is_absolute(),
        "development Web directory must be absolute"
    );
    xcss::web_assets::DirectoryAssets::new(root)?;
    anyhow::ensure!(
        root.join("index.html").is_file() && root.join("assets").is_dir(),
        "development Web directory must contain index.html and assets"
    );
    let mut entries = fs::read_dir(root)?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    anyhow::ensure!(
        entries
            == [
                std::ffi::OsString::from("assets"),
                std::ffi::OsString::from("index.html")
            ],
        "development Web directory must contain only the declared browser entry and assets"
    );
    Ok(fs::canonicalize(root)?)
}

fn decode_key(value: &str) -> anyhow::Result<[u8; 32]> {
    let decoded = STANDARD
        .decode(value.trim())
        .context("XSCS_CREDENTIAL_KEY must be base64")?;
    decoded
        .try_into()
        .map_err(|_| anyhow::anyhow!("XSCS_CREDENTIAL_KEY must decode to exactly 32 bytes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_assets_need_no_external_directory_and_production_rejects_overrides() {
        assert!(development_static_dir(None, false).unwrap().is_none());
        assert!(development_static_dir(Some("/not-needed"), false).is_err());
    }

    #[test]
    fn credential_key_decoding_requires_exactly_32_bytes() {
        assert!(decode_key(&STANDARD.encode([7_u8; 32])).is_ok());
        assert!(decode_key(&STANDARD.encode([7_u8; 31])).is_err());
    }

    #[test]
    fn static_directory_is_absolute_complete_and_contains_no_link_aliases() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("index.html"), "current").unwrap();
        fs::create_dir(directory.path().join("assets")).unwrap();
        fs::write(directory.path().join("assets/app.js"), "current").unwrap();

        assert_eq!(
            validate_static_dir(directory.path().to_str().unwrap(), false).unwrap(),
            directory.path().canonicalize().unwrap()
        );
        assert!(validate_static_dir(directory.path().to_str().unwrap(), true).is_err());
        assert!(validate_static_dir("web/dist", false).is_err());

        fs::remove_file(directory.path().join("index.html")).unwrap();
        assert!(validate_static_dir(directory.path().to_str().unwrap(), false).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn static_directory_rejects_symbolic_and_hard_linked_assets() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let index = directory.path().join("index.html");
        fs::write(&index, "current").unwrap();
        fs::create_dir(directory.path().join("assets")).unwrap();
        fs::write(directory.path().join("assets/app.js"), "current").unwrap();
        let alias = directory.path().join("alias.html");
        fs::hard_link(&index, &alias).unwrap();
        assert!(validate_static_dir(directory.path().to_str().unwrap(), false).is_err());

        fs::remove_file(&alias).unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        symlink(outside.path(), directory.path().join("linked.js")).unwrap();
        assert!(validate_static_dir(directory.path().to_str().unwrap(), false).is_err());
    }
}

fn validate_intrinsic(
    settings: &Settings,
    source: ConfigSource,
) -> Result<(), xcss::config::ConfigError> {
    let invalid =
        |path| xcss::config::ConfigError::new(xcss::config::Reason::InvalidValue, path, source);
    let bind: SocketAddr = settings.bind.parse().map_err(|_| invalid("/bind"))?;
    if !bind.ip().is_loopback() {
        return Err(invalid("/bind"));
    }
    if let Some(key) = &settings.credential_key {
        decode_key(key).map_err(|_| invalid("/credential_key"))?;
    }
    SecretBox::new(settings.credential_key_id.clone(), [0; 32])
        .map_err(|_| invalid("/credential_key_id"))?;
    xcss::admin_auth::normalize_administrator_username(&settings.bootstrap_admin_username)
        .map_err(|_| invalid("/bootstrap_admin_username"))?;
    if let Some(password) = &settings.bootstrap_admin_password {
        xcss::admin_auth::validate_password(password)
            .map_err(|_| invalid("/bootstrap_admin_password"))?;
    }
    if settings
        .data_dir
        .as_ref()
        .is_some_and(|path| !path.is_absolute())
    {
        return Err(invalid("/data_dir"));
    }
    if let Some(url) = &settings.database_url {
        crate::database_schema::database_path(url).map_err(|_| invalid("/database_url"))?;
    }
    if settings
        .development_web_dir
        .as_ref()
        .is_some_and(|path| !Path::new(path).is_absolute())
    {
        return Err(invalid("/development_web_dir"));
    }
    Ok(())
}

#[cfg(test)]
mod precedence_contract_tests {
    use super::*;
    #[test]
    fn a_higher_priority_override_cannot_hide_an_invalid_file_value() {
        let file =
            serde_json::to_vec(&serde_json::json!({"credential_key":"private-invalid-secret"}))
                .unwrap();
        let error = xcss::config::resolve_validated(
            &Settings::default(),
            Some(&file),
            &[],
            &[Override::new("/bind", serde_json::json!("127.0.0.1:18104"))],
            validate_intrinsic,
        )
        .err()
        .expect("invalid lower layer must fail");
        assert_eq!(error.path, "/credential_key");
        assert_eq!(error.source, ConfigSource::File);
        assert!(
            !serde_json::to_string(&error.envelope())
                .unwrap()
                .contains("private-invalid-secret")
        );
    }
}

/// Normalize the CLI file authority before private reads and resource reporting.
pub fn normalize_config_path(path: &Path) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(
        !path
            .components()
            .any(|part| part == std::path::Component::ParentDir),
        "config path cannot contain parent traversal"
    );
    Ok(std::path::absolute(path)?)
}
