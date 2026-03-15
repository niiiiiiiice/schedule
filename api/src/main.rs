use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};
use api::openapi::ApiDoc;
use api::startup::handlers::build_app_state;
use api::startup::router::build_router;
use infrastructure::startup::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "api=debug,infrastructure=debug,application=debug".into()),
        )
        .init();

    let config = Config::from_env();
    let infra = infrastructure::startup::init(&config).await?;
    let state = build_app_state(&infra);

    let app = build_router(state)
        .merge(Scalar::with_url("/scalar", ApiDoc::openapi()))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    let display_addr = config.bind_addr.replace("0.0.0.0", "localhost");
    info!("Server starting on http://{display_addr}");
    info!("Scalar available on http://{display_addr}/scalar");
    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
