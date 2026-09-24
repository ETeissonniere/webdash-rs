use super::{app::App, logo};
use axum::extract::State;
use axum::http::header;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::Router;
use std::sync::Arc;
use tower_http::services::ServeDir;

async fn index(State(app): State<Arc<App>>) -> impl IntoResponse {
    let services = app.services().await;
    (
        [(header::CACHE_CONTROL, "no-cache")],
        Html(home_ui::render_page(&services, app.leptos_options.clone())),
    )
}

pub fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/logo/{key}", get(logo::service_logo))
        .fallback_service(ServeDir::new(app.leptos_options.site_root.as_ref()))
        .with_state(app)
}
