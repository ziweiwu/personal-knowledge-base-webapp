use anyhow::{Context, Result};
use clap::Parser;
use kbviewer_core::config::Config;
use kbviewer_server::auth::store::AuthStore;
use kbviewer_server::{cli, logging, router, state, watch};
use state::AppState;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Cli::parse();
    logging::init(args.log_file.as_deref())?;
    let writes_its_own_log = args.log_file.is_some();

    let outcome = run(args).await;
    // Returning the error prints it to stderr, which is the whole log only when there is
    // no log file; under launchd stderr goes nowhere, so the reason has to be in the file.
    if let Err(error) = &outcome {
        if writes_its_own_log {
            tracing::error!("{error:#}");
        }
    }
    outcome
}

async fn run(args: cli::Cli) -> Result<()> {
    let config = Config::load(&args.config).context("could not load configuration")?;
    let store = AuthStore::open(&config.data_dir).context("could not open the account store")?;

    if let Some(cli::Command::User { action }) = args.command {
        return cli::run_user_command(&store, action);
    }

    refuse_to_start_without_accounts(&store)?;
    warn_about_missing_roots(&config);

    let address: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .context("invalid host or port")?;

    // Claimed before the index is built, not after. A second server wanting the same
    // port (the launch agent and KBViewer.app both default to 4321) then fails at once
    // instead of after indexing the whole vault, and the app's probe finds this server
    // while it is still indexing rather than mistaking the port for a free one.
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("could not bind {address}"))?;

    let state = AppState::new(config, store);
    // Held for the process lifetime; dropping the watchers would stop the watch.
    let _watchers = watch::spawn(state.clone())?;

    serve(router::build(state), listener).await
}

/// An empty account store with a login page in front of it is a locked door with no key,
/// and starting anyway invites someone to "fix" it by disabling authentication.
fn refuse_to_start_without_accounts(store: &AuthStore) -> Result<()> {
    if store.user_count() == 0 {
        anyhow::bail!(
            "no accounts exist yet. Create one first:\n    kbviewer user add you@example.com"
        );
    }
    Ok(())
}

fn warn_about_missing_roots(config: &Config) {
    for root in &config.roots {
        if !root.path.is_dir() {
            tracing::warn!(
                root = %root.id,
                path = %root.path.display(),
                "configured folder does not exist"
            );
        }
    }
}

async fn serve(app: axum::Router, listener: tokio::net::TcpListener) -> Result<()> {
    let address = listener
        .local_addr()
        .context("could not read the bound address")?;
    tracing::info!(%address, "kbviewer listening");
    eprintln!("kbviewer listening on http://{address}");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .context("server error")
}
