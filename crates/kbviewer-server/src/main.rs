use anyhow::{Context, Result};
use clap::Parser;
use kbviewer_core::config::Config;
use kbviewer_server::auth::store::AuthStore;
use kbviewer_server::{cli, router, state, watch};
use state::AppState;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let args = cli::Cli::parse();
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

    let state = AppState::new(config, store);
    // Held for the process lifetime; dropping the debouncers would stop the watch.
    let _watchers = watch::spawn(state.clone())?;

    serve(router::build(state), address).await
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "kbviewer=info,tower_http=warn".into()),
        )
        .init();
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

async fn serve(app: axum::Router, address: SocketAddr) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("could not bind {address}"))?;

    tracing::info!(%address, "kbviewer listening");
    eprintln!("kbviewer listening on http://{address}");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .context("server error")
}
