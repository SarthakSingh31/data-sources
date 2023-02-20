use std::{env, sync::Arc};

use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect, Response},
    routing, Json,
};
use sheets::{spreadsheets::Spreadsheets, AccessToken, Client};
use sqlx::{Connection, Executor};
use tokio::sync::RwLock;

/// The enviorment variable used to get the port of the server
const PORT_ENV_VAR: &'static str = "GSHEET_DB_SYNC_PORT";
/// The enviorment variable used to get the redirect uri of the login
const REDIRECT_URI_ENV_VAR: &'static str = "GSHEET_DB_SYNC_REDIRECT_URI";
/// The default port which the webserver will bind to. Also the default port of the redirect uri for OAuth.
const DEFAULT_PORT: &'static str = "8889";

/// The  enviorment variable used to get client id for gcp login
const CLIENT_ID_ENV_VAR: &'static str = "GSHEET_DB_SYNC_CLIENT_ID";
/// The  enviorment variable used to get client secret for gcp login
const CLIENT_SECRET_ENV_VAR: &'static str = "GSHEET_DB_SYNC_CLIENT_SECRET";

struct AppState {
    client: RwLock<Client>,
    user_consent_url: String,
    access_token: RwLock<Option<AccessToken>>,
}

#[tokio::main]
async fn main() {
    // This client is used to do the oauth authentication with google
    let client = Client::new(
        env::var(CLIENT_ID_ENV_VAR).unwrap(),
        env::var(CLIENT_SECRET_ENV_VAR).unwrap(),
        env::var(REDIRECT_URI_ENV_VAR).unwrap_or(format!(
            "http://127.0.0.1:{}",
            env::var(PORT_ENV_VAR).unwrap_or(DEFAULT_PORT.to_string())
        )),
        "",
        "",
    );

    let user_consent_url =
        client.user_consent_url(&["https://www.googleapis.com/auth/spreadsheets".to_string()]);

    let app = axum::Router::new()
        .route("/", routing::get(index))
        .route("/fetch_spreadsheet", routing::get(fetch_spreadsheet))
        .with_state(Arc::new(AppState {
            client: RwLock::new(client),
            user_consent_url,
            access_token: RwLock::new(None),
        }))
        .route("/sync_spreadsheet", routing::post(sync_spreadsheet));

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

#[derive(serde::Deserialize)]
struct Auth {
    code: Option<String>,
    state: Option<String>,
}

impl Auth {
    fn is_none(&self) -> bool {
        self.code.is_none() || self.state.is_none()
    }
}

/// The index page of the website. If it detects that the OAuth authentication has not been
/// done yet it will redirect to the authentication workflow.
/// Also handles the returned values from the redirect uri.
/// In the case where authentication is done it serves the index.html file
async fn index(State(state): State<Arc<AppState>>, Query(query): Query<Auth>) -> Response {
    if state.access_token.read().await.is_none() && query.is_none() {
        Redirect::to(&state.user_consent_url).into_response()
    } else if !query.is_none() {
        *state.access_token.write().await = Some(
            state
                .client
                .write()
                .await
                .get_access_token(&query.code.unwrap(), &query.state.unwrap())
                .await
                .unwrap(),
        );

        Redirect::to("/").into_response()
    } else {
        Html(include_str!("index.html")).into_response()
    }
}

#[derive(serde::Deserialize)]
pub struct FetchArg {
    id: String,
}

/// Fetches the first spreadsheet associated with the id passed in [`FetchArg`].
/// Returns the fetched data as json.
async fn fetch_spreadsheet(
    State(state): State<Arc<AppState>>,
    Query(arg): Query<FetchArg>,
) -> Json<Vec<Vec<CellValue>>> {
    let spreadsheet = Spreadsheets {
        client: state.client.read().await.clone(),
    }
    .get(&arg.id, true, &[])
    .await
    .expect("Failed to fetch spreadsheet");

    // Converts the data in the spreadsheet to a 2d array of cell values
    let values: Vec<Vec<_>> = spreadsheet.sheets[0].data[0]
        .row_data
        .iter()
        .map(|row| {
            row.values
                .iter()
                .map(|cell| {
                    cell.effective_value
                        .clone()
                        .map(|cell| {
                            if !cell.string_value.is_empty() {
                                CellValue::String(cell.string_value)
                            } else if cell.bool_value {
                                CellValue::Bool(cell.bool_value)
                            } else {
                                CellValue::Number(cell.number_value)
                            }
                        })
                        .unwrap_or(CellValue::Null)
                })
                .collect()
        })
        .collect();

    Json(values)
}

#[derive(serde::Deserialize)]
pub struct SyncArg {
    /// The id of the spreadsheet
    id: String,
    /// The uri of the database
    db: String,
}

/// Syncs the database to match the spreadsheet data
async fn sync_spreadsheet(
    Query(arg): Query<SyncArg>,
    Json(spreadsheet): Json<Vec<Vec<CellValue>>>,
) {
    let mut conn = sqlx::any::AnyConnection::connect(&arg.db)
        .await
        .expect("Failed to connect to DB");

    // Remove the old table and a fresh one with the new data
    let drop_table = format!("DROP TABLE '{}';", arg.id);
    let _ = conn.fetch_optional(drop_table.as_str()).await;

    // Determine which fields the new table will require
    let fields = spreadsheet[0]
        .iter()
        .enumerate()
        .map(|(idx, cell)| {
            let is_value_string = spreadsheet[1..].iter().any(|row| {
                if let CellValue::String(_) = &row[idx] {
                    true
                } else {
                    false
                }
            });

            if let CellValue::String(s) = cell {
                format!(
                    "{s} {}",
                    if is_value_string {
                        "varchar(1000)"
                    } else {
                        "number"
                    }
                )
            } else {
                format!(
                    "{} {}",
                    (('a' as u8 + idx as u8) as char),
                    if is_value_string {
                        "varchar(1000)"
                    } else {
                        "number"
                    }
                )
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    let create_table = format!("CREATE TABLE '{}' ({fields});", arg.id);
    let _ = conn.execute(create_table.as_str()).await;

    // Insert all the rows into the table. (The first one is skipped as it is treated as the table heading)
    let values = spreadsheet
        .into_iter()
        .skip(1)
        .map(|row| {
            row.iter()
                .map(|cell| match cell {
                    CellValue::Number(num) => format!("{num}"),
                    CellValue::String(s) => format!("'{s}'"),
                    CellValue::Bool(b) => format!("{b}"),
                    CellValue::Null => "NULL".to_owned(),
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .map(|value| format!("({value})"))
        .collect::<Vec<_>>()
        .join(",");

    let insert = format!("INSERT INTO '{}' VALUES {values}", arg.id);
    let _ = conn.execute(insert.as_str()).await;
}

// Different types of values a google spreadsheet cell can have
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
enum CellValue {
    Number(f64),
    String(String),
    Bool(bool),
    Null,
}
