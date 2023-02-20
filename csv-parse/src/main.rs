use std::env;

use axum::{
    extract::{Multipart, Query},
    response::{Html, IntoResponse, Response},
    routing, Json,
};
use sqlx::{Connection, Executor};

/// The enviorment variable used to get the port of the server
const PORT_ENV_VAR: &'static str = "CSV_PARSE_PORT";
/// The default port which the webserver will bind to
const DEFAULT_PORT: &'static str = "8888";

#[tokio::main]
async fn main() {
    let app = axum::Router::new()
        .route("/", routing::get(index))
        .route("/csv_to_json", routing::post(csv_to_json))
        .route("/upload_to_db", routing::post(upload_to_db));

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

/// The value representation of a single cell in the CSV
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
enum Cell {
    String(String),
    Int(i64),
    Float(f64),
    Null,
}

/// A complete Crable
#[derive(serde::Serialize, serde::Deserialize)]
struct Table {
    /// The heading of each column
    headers: Vec<String>,
    /// All the data present in a csv
    data: Vec<Vec<Cell>>,
}

/// Convert the sent csv file into json of [`Table`]
async fn csv_to_json(mut multipart: Multipart) -> Json<Table> {
    let field = multipart
        .next_field()
        .await
        .expect("No fields in the form")
        .expect("Field was null");
    let field_data = field.bytes().await.expect("Failed to read field bytes");

    let mut reader = csv::Reader::from_reader(field_data.as_ref());

    Json(Table {
        headers: reader
            .headers()
            .expect("Failed to parse header")
            .into_iter()
            .map(|header| header.to_owned())
            .collect(),
        data: reader
            .deserialize::<Vec<Cell>>()
            .map(|row| row.expect("Failed to parse row"))
            .collect(),
    })
}

#[derive(serde::Deserialize)]
struct UploadArgs {
    table_name: String,
    db_uri: String,
}

/// Uploads the sent [`Table`] to a db based on the params in [`UploadArgs`]
async fn upload_to_db(Query(args): Query<UploadArgs>, Json(table): Json<Table>) {
    let mut conn = sqlx::AnyConnection::connect(&args.db_uri)
        .await
        .expect("Failed to connect to DB");

    // Remove the old table and a fresh one with the new data
    let drop_table = format!("DROP TABLE '{}';", args.table_name);
    let _ = conn.fetch_optional(drop_table.as_str()).await;

    // Determine which fields the new table will require
    let fields = table.headers.join(",");
    let create_table = format!("CREATE TABLE '{}' ({fields});", args.table_name);
    let _ = conn.execute(create_table.as_str()).await;

    // Insert all the rows into the table
    let values = table
        .data
        .into_iter()
        .map(|row| {
            row.iter()
                .map(|cell| match cell {
                    Cell::String(s) => format!("'{s}'"),
                    Cell::Int(i) => format!("{i}"),
                    Cell::Float(f) => format!("{f}"),
                    Cell::Null => format!("NULL"),
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .map(|value| format!("({value})"))
        .collect::<Vec<_>>()
        .join(",");

    let insert = format!("INSERT INTO '{}' VALUES {values}", args.table_name);
    let _ = conn.execute(insert.as_str()).await;
}
