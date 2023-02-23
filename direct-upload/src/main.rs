use std::env;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse, Response};
use axum::routing;
use google_cloud_default::WithAuthExt;
use google_cloud_storage::client::{Client, ClientConfig};
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
    let config = ClientConfig::default()
        .with_auth()
        .await
        .expect("Failed to get GCP authentication files");
    let client = Client::new(config);

    let app = axum::Router::new()
        .route("/", routing::get(index))
        .route("/get_signed_url", routing::get(get_signed_url))
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

async fn index() -> Response {
    Html(include_str!("index.html")).into_response()
}

#[derive(serde::Deserialize)]
struct SigningArgs {
    name: String,
}

async fn get_signed_url(
    State(state): State<Arc<AppState>>,
    Query(args): Query<SigningArgs>,
) -> Response {
    state
        .client
        .signed_url(
            "test-bucket-the-first",
            &args.name,
            SignedURLOptions {
                method: SignedURLMethod::PUT,
                ..Default::default()
            },
        )
        .await
        .expect("Failed to generate the signing url")
        .into_response()
}
