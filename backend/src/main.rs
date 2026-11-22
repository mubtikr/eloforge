use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Context;
use axum::{
    http::{
        header::{ACCEPT, AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, REFERRER_POLICY},
        HeaderName, HeaderValue, Method,
    },
    Router,
};
use eloforge_api::{
    auth::{AuthProvider, ChessComChallengeProvider, ChessComOAuthProvider},
    chesscom::ChessComClient,
    config::Config,
    db,
    engine::EnginePool,
    events::EventBus,
    metrics::Metrics,
    models::UserDto,
    routes,
    state::AppState,
    sync::SyncService,
};
use sqlx::PgPool;
use tokio::signal;
use tower_http::{
    compression::CompressionLayer,
    cors::{AllowOrigin, CorsLayer},
    limit::RequestBodyLimitLayer,
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let cfg = Config::from_env().context("invalid configuration")?;
    tracing::info!(
        model = %cfg.opencode_model,
        protocol = cfg.opencode_protocol.as_str(),
        workers = cfg.stockfish_workers,
        "starting EloForge API"
    );

    let db = connect_with_retry(&cfg.database_url, 40).await?;
    db::migrate(&db)
        .await
        .context("database migration failed")?;
    tracing::info!("database ready and migrated");

    let redis_client =
        redis::Client::open(cfg.valkey_url.as_str()).context("invalid VALKEY_URL")?;
    let redis = connect_valkey(&redis_client, 40).await?;
    tracing::info!("valkey ready");

    let cfg = Arc::new(cfg);
    let metrics = Metrics::new()?;
    let engine = EnginePool::new(&cfg, metrics.clone());
    let events = EventBus::new(redis_client, redis.clone()).await;
    let chesscom = Arc::new(ChessComClient::new(&cfg)?);

    let auth: Arc<dyn AuthProvider> = Arc::new(ChessComChallengeProvider::new(
        db.clone(),
        chesscom.clone(),
        redis.clone(),
        &cfg,
    ));
    let oauth = Arc::new(ChessComOAuthProvider::new(&cfg));
    let sync = SyncService::new(
        db.clone(),
        chesscom.clone(),
        redis.clone(),
        events.clone(),
        cfg.clone(),
        metrics.clone(),
    );

    let state = AppState {
        cfg: cfg.clone(),
        db,
        redis,
        events,
        engine,
        metrics,
        http: reqwest::Client::builder()
            .user_agent("EloForge/1.0")
            .timeout(Duration::from_secs(30))
            .build()?,
        chesscom,
        auth,
        oauth,
        sync,
        started_at: std::time::Instant::now(),
    };

    eloforge_api::analyzer::spawn_workers(state.clone(), 1);
    spawn_sync_scheduler(state.clone());

    let app = build_app(state.clone());

    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind {addr}"))?;
    tracing::info!(%addr, "EloForge API listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;

    tracing::info!("EloForge API stopped");
    Ok(())
}

fn spawn_sync_scheduler(state: AppState) {
    tokio::spawn(async move {
        let period = Duration::from_secs(state.cfg.sync_interval_minutes.max(1) * 60);
        let mut ticker = tokio::time::interval(period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        ticker.tick().await;
        loop {
            ticker.tick().await;
            let users = sqlx::query_as::<_, UserDto>(
                "SELECT id, chesscom_username, display_name, country, avatar_url, title,
                        current_elo, peak_elo, created_at
                 FROM users ORDER BY created_at ASC LIMIT 500",
            )
            .fetch_all(&state.db)
            .await;
            match users {
                Ok(users) => {
                    for user in users {
                        match state.sync.trigger(&user).await {
                            Ok(true) => {
                                tracing::info!(user_id = %user.id, "scheduled sync started");
                                tokio::time::sleep(Duration::from_secs(3)).await;
                            }
                            Ok(false) => {}
                            Err(err) => {
                                tracing::warn!(error = %err, user_id = %user.id, "scheduled sync trigger failed")
                            }
                        }
                    }
                }
                Err(err) => tracing::warn!(error = %err, "scheduled sync user query failed"),
            }
        }
    });
}

fn build_app(state: AppState) -> Router {
    let origin = state
        .cfg
        .frontend_origin
        .parse::<HeaderValue>()
        .unwrap_or_else(|_| HeaderValue::from_static("http://localhost:5173"));

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::exact(origin))
        .allow_credentials(true)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            ACCEPT,
            AUTHORIZATION,
            CONTENT_TYPE,
            HeaderName::from_static("x-requested-with"),
        ]);

    let security_headers = tower::ServiceBuilder::new()
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
        ));

    Router::new()
        .merge(routes::router())
        .layer(security_headers)
        .layer(cors)
        .layer(CompressionLayer::new())
        .layer(RequestBodyLimitLayer::new(2 * 1024 * 1024))
        .layer(SetResponseHeaderLayer::overriding(
            CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let json = std::env::var("LOG_FORMAT")
        .map(|v| v.eq_ignore_ascii_case("json"))
        .unwrap_or(false);

    if json {
        tracing_subscriber::registry()
            .with(filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_current_span(true)
                    .with_span_list(true),
            )
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer().compact())
            .init();
    }
}

async fn connect_with_retry(url: &str, attempts: u32) -> anyhow::Result<PgPool> {
    let mut last_err = None;
    for attempt in 1..=attempts {
        match db::connect(url).await {
            Ok(pool) => return Ok(pool),
            Err(err) => {
                tracing::warn!(attempt, error = %err, "database not ready; retrying");
                last_err = Some(err);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("database connection failed")))
}

async fn connect_valkey(
    client: &redis::Client,
    attempts: u32,
) -> anyhow::Result<redis::aio::MultiplexedConnection> {
    let mut last_err = None;
    for attempt in 1..=attempts {
        match client.get_multiplexed_async_connection().await {
            Ok(conn) => return Ok(conn),
            Err(err) => {
                tracing::warn!(attempt, error = %err, "valkey not ready; retrying");
                last_err = Some(err);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
    Err(anyhow::anyhow!(
        "valkey connection failed: {}",
        last_err
            .map(|e| e.to_string())
            .unwrap_or_else(|| "unknown".into())
    ))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(err) => {
                tracing::warn!(error = %err, "failed to install SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
