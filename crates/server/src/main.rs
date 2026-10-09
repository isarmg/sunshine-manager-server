use std::{io::Read as _, path::PathBuf};

use clap::{Parser, Subcommand};
use xcss_admin_core::AdministratorStore;
use xscs::{
    ServeConfig, db,
    http::{WorkerState, router},
    release_bundle, release_contract,
    runtime_lock::{ApplicationLock, MaintenanceLock},
};

#[derive(Parser)]
#[command(
    name = "xscs",
    version,
    long_version = concat!(env!("CARGO_PKG_VERSION"), " source=", env!("XSCS_SOURCE_REVISION"), " foundation=", env!("XCSS_FOUNDATION_REVISION")),
    about = "Independent Sunshine manager"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    #[arg(long, global = true)]
    bind: Option<std::net::SocketAddr>,
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Explicitly initialize a private data directory and first administrator.
    Init,
    /// Run a previously initialized instance.
    Run {
        #[arg(long)]
        release_root: Option<PathBuf>,
    },
    /// Inspect the current configuration and data without changing either.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Query the live service readiness.
    Status,
    /// Reset an existing local administrator password.
    AdminResetPassword(AdminResetPasswordArgs),
    /// Run a deployment health check against the configured instance.
    Doctor,
    /// Print the exact machine-readable product, API, schema and target identity.
    Identity,
    /// Print the browser asset inventory compiled into this binary.
    WebAssets,
    /// Verify an immutable release tree with the binary contained in that tree.
    VerifyRelease(VerifyReleaseArgs),
}

#[derive(Subcommand)]
enum ConfigCommand {
    Validate,
}

#[derive(clap::Args)]
struct AdminResetPasswordArgs {
    #[arg(long, hide_env_values = true)]
    database_url: String,
    #[arg(long)]
    username: String,
}

#[derive(clap::Args)]
struct VerifyReleaseArgs {
    #[arg(long)]
    root: PathBuf,
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let json = std::env::args_os().any(|argument| argument == "--json");
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            return if error.print().is_ok() {
                std::process::ExitCode::SUCCESS
            } else {
                std::process::ExitCode::FAILURE
            };
        }
        Err(_) => {
            let error = xcss_server_cli::ErrorEnvelope::with_code(
                xcss_server_cli::ErrorCode::new("invalid_cli_input").unwrap(),
                "Command arguments do not satisfy the current CLI contract; use --help.",
            );
            return xcss_server_cli::report_error(&error, json, 2);
        }
    };
    match execute(cli).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            let envelope = if let Some(error) = error.downcast_ref::<xcss_config::ConfigError>() {
                error.envelope()
            } else if let Some(error) = error.downcast_ref::<xcss_server_cli::CliError>() {
                error.0.clone()
            } else if let Some(error) = error.downcast_ref::<xcss_state_file::Error>() {
                xcss_server_cli::state_error(error)
            } else if let Some(error) = error.downcast_ref::<xcss_server_cli::SnapshotError>() {
                xcss_server_cli::snapshot_error(error)
            } else {
                if !json {
                    eprintln!("{error:#}");
                }
                xcss_server_cli::ErrorEnvelope::with_code(
                    xcss_server_cli::ErrorCode::new("current_state_invalid").unwrap(),
                    "The command could not validate or operate on the current configuration and data.",
                )
            };
            xcss_server_cli::report_error(&envelope, json, 1)
        }
    }
}

async fn query_status(bind: std::net::SocketAddr, json: bool) -> anyhow::Result<()> {
    let report = xcss_server_cli::query_status(bind, "xscs")
        .await
        .map_err(xcss_server_cli::CliError)?;
    if !report.ready {
        return Err(
            xcss_server_cli::CliError(xcss_server_cli::ErrorEnvelope::with_code(
                xcss_server_cli::ErrorCode::new("service_not_ready").unwrap(),
                "The service answered but its business readiness checks failed.",
            ))
            .into(),
        );
    }
    xcss_server_cli::print_report(&report, json)?;
    Ok(())
}

async fn execute(mut cli: Cli) -> anyhow::Result<()> {
    cli.config = cli
        .config
        .as_deref()
        .map(xscs::config::normalize_config_path)
        .transpose()?;
    initialize_logging()?;
    let configuration =
        || ServeConfig::from_sources(cli.config.as_deref(), cli.data_dir.as_deref(), cli.bind);
    match cli.command.unwrap_or(Command::Run { release_root: None }) {
        Command::Init => {
            let config = configuration()?;
            let password = config
                .bootstrap_admin_password
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("bootstrap_admin_password is required for init"))?;
            xcss_admin_auth::validate_password(password)?;
            xcss_server_cli::runtime_allowed(&config.data_dir)
                .map_err(xcss_server_cli::CliError)?;
            xcss_server_cli::create_empty_private_directory(&config.data_dir)
                .map_err(xcss_server_cli::CliError)?;
            let directory = xcss_state_file::PrivateStateDirectory::open(&config.data_dir)?;
            xcss_server_cli::runtime_allowed(directory.path())
                .map_err(xcss_server_cli::CliError)?;
            let _maintenance = directory.try_maintenance_lock()?;
            xcss_server_cli::runtime_allowed(directory.path())
                .map_err(xcss_server_cli::CliError)?;
            xcss_server_cli::create_runtime_log_directory(&config.data_dir)
                .map_err(xcss_server_cli::CliError)?;
            let database = xscs::database_schema::database_path(&config.database_url)?;
            anyhow::ensure!(
                !database.try_exists()?,
                "database already exists; init never overwrites existing data"
            );
            let pool = db::open_or_initialize(&config.database_url).await?;
            let service = xcss_admin_core::AdministratorService::new(
                xcss_admin_sqlite::SqliteAdministratorStore::new(pool.clone()),
            );
            service
                .bootstrap_administrator(
                    &config.bootstrap_admin_username,
                    password,
                    current_time_micros()?,
                )
                .await?;
            db::require_current_runtime_state(&pool, &config.secrets).await?;
            xcss_sqlite::checkpoint(&pool).await?;
            pool.close().await;
            tracing::info!(event = "common.initialization.completed");
            println!(
                "{}",
                serde_json::json!({"status":"initialized", "ready":false})
            );
            Ok(())
        }
        Command::Run { release_root } => {
            let release_root = release_root
                .as_deref()
                .map(release_bundle::resolve_run_root)
                .transpose()?;
            if let Some(root) = &release_root {
                release_bundle::verify_release(root)?;
            } else {
                anyhow::ensure!(
                    !release_contract::BinaryIdentity::current()?.is_release_bound(),
                    "source-bound release binaries require run --release-root"
                );
            }
            serve_with_config(configuration()?, release_root.as_deref()).await
        }
        Command::Config {
            command: ConfigCommand::Validate,
        } => {
            let config = configuration()?;
            validate_existing_configuration(&config).await?;
            println!(
                "{}",
                serde_json::json!({"status":"valid", "state_paths":std::iter::once(config.data_dir.clone()).chain(cli.config.as_ref().map(|path| path.canonicalize()).transpose()?).collect::<Vec<_>>(), "sources":config.sources, "schema_identity":xscs::database_schema::current_schema_identity()})
            );
            Ok(())
        }
        Command::Status => {
            let config = configuration()?;
            query_status(config.bind, cli.json).await
        }
        Command::AdminResetPassword(args) => {
            let password = read_password_from_stdin()?;
            let database = xscs::database_schema::database_path(&args.database_url)?;
            let directory =
                xcss_state_file::PrivateStateDirectory::open(database.parent().unwrap())?;
            xcss_server_cli::runtime_allowed(directory.path())
                .map_err(xcss_server_cli::CliError)?;
            let _common_lock = directory.try_maintenance_lock()?;
            xcss_server_cli::runtime_allowed(directory.path())
                .map_err(xcss_server_cli::CliError)?;
            xscs::database_schema::validate_configuration_database(&args.database_url)?;
            let maintenance = MaintenanceLock::exclusive(&args.database_url)?;
            let pool =
                xscs::database_schema::open_validated_location(&maintenance.database_url()).await?;
            let username = xcss_admin_auth::normalize_administrator_username(&args.username)?;
            let service = xcss_admin_core::AdministratorService::new(
                xcss_admin_sqlite::SqliteAdministratorStore::new(pool),
            );
            service
                .change_administrator_password(&username, &password, current_time_micros()?)
                .await
                .map_err(|error| anyhow::anyhow!(error))?;
            println!(
                "{{\"status\":\"password-reset\",\"username\":{:?}}}",
                username
            );
            Ok(())
        }
        Command::Doctor => {
            let config = configuration()?;
            let directory = xcss_state_file::PrivateStateDirectory::open(&config.data_dir)?;
            xcss_server_cli::runtime_allowed(directory.path())
                .map_err(xcss_server_cli::CliError)?;
            let _common_lock = directory.try_maintenance_lock()?;
            xcss_server_cli::runtime_allowed(directory.path())
                .map_err(xcss_server_cli::CliError)?;
            xscs::database_schema::validate_configuration_database(&config.database_url)?;
            let maintenance = MaintenanceLock::exclusive(&config.database_url)?;
            let pool =
                xscs::database_schema::open_validated_location(&maintenance.database_url()).await?;
            let report = db::doctor(&pool, &config.secrets).await;
            println!(
                "{{\"status\":\"{}\",\"bind\":\"{}\",\"schema_ready\":{},\
                 \"integrity_ready\":{},\"foreign_keys_ready\":{},\"writable\":{},\
                 \"encrypted_values_ready\":{}}}",
                if report.healthy() { "ok" } else { "degraded" },
                config.bind,
                report.schema_ready,
                report.integrity_ready,
                report.foreign_keys_ready,
                report.writable,
                report.encrypted_values_ready,
            );
            if !report.healthy() {
                anyhow::bail!("xscs doctor found an unhealthy local boundary");
            }
            Ok(())
        }
        Command::WebAssets => {
            xscs::web_assets::verify()?;
            println!("{}", xscs::web_assets::MANIFEST);
            Ok(())
        }
        Command::Identity => {
            println!("{}", release_contract::current_json()?);
            Ok(())
        }
        Command::VerifyRelease(args) => {
            let report = release_bundle::verify_release(&args.root)?;
            println!("{}", serde_json::to_string(&report)?);
            Ok(())
        }
    }
}

fn read_password_from_stdin() -> anyhow::Result<String> {
    let mut bytes = Vec::new();
    std::io::stdin().take(1025).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= 1024, "password input exceeds 1024 bytes");
    let mut password = String::from_utf8(bytes)?;
    if password.ends_with('\n') {
        password.pop();
        if password.ends_with('\r') {
            password.pop();
        }
    }
    anyhow::ensure!(
        !password.contains(['\r', '\n']),
        "password input must contain exactly one line"
    );
    Ok(password)
}

async fn serve_with_config(
    config: ServeConfig,
    release_root: Option<&std::path::Path>,
) -> anyhow::Result<()> {
    if release_root.is_some() {
        anyhow::ensure!(
            config.static_dir.is_none(),
            "formal releases require embedded Web assets"
        );
    }
    let signals = xcss_server_runtime::ProcessSignals::install()?;
    let listeners = xcss_server_runtime::BoundListeners::bind([config.bind])?;
    let transport = xcss_server_runtime::HttpServer::new(listeners, signals);
    // Hold both locks for the complete process lifetime. The instance lock
    // rejects a second worker, while the shared maintenance lock excludes
    // exclusive state and administrator maintenance.
    xcss_server_cli::runtime_allowed(&config.data_dir).map_err(xcss_server_cli::CliError)?;
    validate_existing_configuration(&config).await?;
    let directory = xcss_state_file::PrivateStateDirectory::open(&config.data_dir)?;
    xcss_server_cli::runtime_allowed(directory.path()).map_err(xcss_server_cli::CliError)?;
    let _common_lock = directory.try_instance_lock()?;
    xcss_server_cli::runtime_allowed(directory.path()).map_err(xcss_server_cli::CliError)?;
    let application_lock = ApplicationLock::acquire(&config.database_url)?;
    validate_existing_configuration(&config).await?;
    enable_runtime_logging(&config.data_dir)?;
    tracing::info!(event = "common.config.loaded");
    xscs::database_schema::validate_configuration_database(&config.database_url)?;
    let pool =
        xscs::database_schema::open_validated_location(&application_lock.database_url()).await?;
    db::require_current_runtime_state(&pool, &config.secrets).await?;
    let administrator_service = xcss_admin_core::AdministratorService::new(
        xcss_admin_sqlite::SqliteAdministratorStore::new(pool.clone()),
    );
    anyhow::ensure!(
        administrator_service.store().administrator_count().await? > 0,
        "service is not initialized; run init to create the first administrator"
    );
    administrator_service
        .store()
        .validate_all_administrators()
        .await?;
    let state = WorkerState::new(pool, config.secrets, config.production, config.static_dir)?;
    let recovered = state.operation_manager().recover_startup().await?;
    if let Err(error) = state.operation_manager().deliver_outbox().await {
        tracing::warn!(%error, "initial audit outbox delivery failed; background retry will continue");
    }
    let health_pool = state.pool.clone();
    let operation_manager = state.operation_manager().clone();
    let audit_pool = state.pool.clone();
    let operations_pool = state.pool.clone();
    let runtime =
        xcss_server_runtime::ServerRuntime::builder(xcss_server_runtime::ProductDescriptor {
            id: "xscs".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            foundation_revision: env!("XCSS_FOUNDATION_REVISION").to_owned(),
            profile: "server-control-plane".to_owned(),
            capabilities: vec![
                "embedded-web".into(),
                "admin-persistent".to_owned(),
                "server-runtime".to_owned(),
                "server-health".to_owned(),
                "durable-operations".to_owned(),
                "secret-envelope".to_owned(),
            ],
        })
        .with_schema_identity(xscs::database_schema::current_schema_identity())
        .register_metric(
            xcss_server_runtime::DiagnosticMetric::AuditBacklog,
            move || {
                let store = xcss_operations::SqliteOperationStore::new(audit_pool.clone());
                async move { store.pending_audit_count().await.ok() }
            },
        )
        .register_metric(
            xcss_server_runtime::DiagnosticMetric::OperationBacklog,
            move || {
                let store = xcss_operations::SqliteOperationStore::new(operations_pool.clone());
                async move { store.active_operation_count().await.ok() }
            },
        )
        .register_health_check(
            "database",
            xcss_server_runtime::health_check(move || {
                let pool = health_pool.clone();
                async move { db::ready(&pool).await }
            }),
        )
        .register_background_task(
            "durable-operations",
            xcss_server_runtime::TaskCriticality::Critical,
            move |shutdown| operation_manager.run_until(shutdown),
        )
        .register_background_task(
            "shutdown-log",
            xcss_server_runtime::TaskCriticality::Degrading,
            |mut shutdown| async move {
                if !*shutdown.borrow() {
                    let _ = shutdown.changed().await;
                }
                tracing::info!(event = "common.runtime.shutdown_started");
                Ok(())
            },
        )
        .build()
        .await?;
    let runtime_handle = runtime.handle();
    tracing::info!(event = "common.runtime.started");
    tracing::info!(
        bind = %config.bind,
        schema = db::SCHEMA,
        recovered_running_operations = recovered,
        "Sunshine manager ready"
    );
    runtime
        .serve(
            transport,
            router(state, runtime_handle)?.layer(axum::middleware::from_fn_with_state(
                "xscs".to_owned(),
                xcss_server_cli::service_identity_middleware,
            )),
        )
        .await?;
    tracing::info!(event = "common.runtime.stopped");
    Ok(())
}

fn current_time_micros() -> anyhow::Result<u64> {
    use std::time::{SystemTime, UNIX_EPOCH};
    u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_micros())
        .map_err(|_| anyhow::anyhow!("current time exceeds administrator timestamp range"))
}

static LOG_LAYER: std::sync::OnceLock<xcss_log::FoundationStructuredLayer> =
    std::sync::OnceLock::new();
fn initialize_logging() -> anyhow::Result<()> {
    use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};
    let layer = xcss_log::FoundationStructuredLayer::new("xscs")?;
    LOG_LAYER
        .set(layer.clone())
        .map_err(|_| anyhow::anyhow!("logging already initialized"))?;
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(layer)
        .try_init()?;
    Ok(())
}
fn enable_runtime_logging(data_dir: &std::path::Path) -> anyhow::Result<()> {
    xcss_server_cli::validate_runtime_log_directory(data_dir).map_err(xcss_server_cli::CliError)?;
    let file = xcss_log::RotatingLogFile::open(
        data_dir.join("logs"),
        "xscs",
        xcss_log::LogRetention::default(),
    )?;
    LOG_LAYER
        .get()
        .ok_or_else(|| anyhow::anyhow!("logging is unavailable"))?
        .set_rotating_file(file)?;
    Ok(())
}

async fn validate_existing_configuration(config: &ServeConfig) -> anyhow::Result<()> {
    xcss_server_cli::validate_runtime_log_directory(&config.data_dir)
        .map_err(xcss_server_cli::CliError)?;
    xcss_state_file::PrivateStateDirectory::open(&config.data_dir)?;
    let snapshot = xscs::database_schema::validation_snapshot(
        xscs::database_schema::database_path(&config.database_url)?,
    )
    .await?;
    xscs::database_schema::validate_pool(snapshot.pool()).await?;
    db::require_current_runtime_state(snapshot.pool(), &config.secrets).await?;
    let administrator = xcss_admin_sqlite::SqliteAdministratorStore::new(snapshot.pool().clone());
    anyhow::ensure!(
        administrator.administrator_count().await? > 0,
        "administrator initialization is required"
    );
    administrator.validate_all_administrators().await?;
    snapshot.close().await;
    Ok(())
}
