use std::env;
use std::sync::Arc;

use axum::extract::State;
use axum::response::{Html, IntoResponse, Response};
use axum::routing;
use google_cloud_storage::client::Client;
use google_cloud_storage::sign::{SignedURLMethod, SignedURLOptions};

/// The enviorment variable used to get the port of the server
const PORT_ENV_VAR: &'static str = "DIRECT_UPLOAD_PORT";
/// The default port which the webserver will bind to. Also the default port of the redirect uri for OAuth.
const DEFAULT_PORT: &'static str = "8890";

struct AppState {
    client: Client,
}

#[tokio::main]
async fn main() {
    let client = Client::default()
        .await
        .expect("Failed to create gcp client");

    let app = axum::Router::new()
        .route("/", routing::get(index))
        .with_state(Arc::new(AppState { client }));

    axum::Server::bind(
        &format!(
            "0.0.0.0:{}",
            env::var(PORT_ENV_VAR).unwrap_or(DEFAULT_PORT.to_string())
        )
        .parse()
        .unwrap(),
    )
    .serve(app.into_make_service())
    .await
    .unwrap();
}

async fn index(State(state): State<Arc<AppState>>) -> Response {
    let html = include_str!("index.html");
    let signed_url = state
        .client
        .signed_url(
            "test-bucket-the-first",
            "",
            SignedURLOptions {
                method: SignedURLMethod::PUT,
                ..Default::default()
            },
        )
        .await
        .expect("Failed to generate the signing url");

    Html(html.replace("{PUT_URL}", &signed_url)).into_response()
}
