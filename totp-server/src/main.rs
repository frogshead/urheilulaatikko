use anyhow::Result;
use totp_server::{config::Config, db, routes, state::AppState};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,totp_server=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;
    tracing::info!(issuer = %config.keycloak_issuer, "starting the TOTP server");

    let pool = db::create_pool(&config.database_url).await?;
    db::run_migrations(&pool).await?;
    tracing::info!("database migrations applied");

    let address = config.server_address();
    let state = AppState::new(pool, config)?;
    let app = routes::create_router(state)?.layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&address).await?;
    tracing::info!(%address, "listening");

    axum::serve(listener, app).await?;

    Ok(())
}
