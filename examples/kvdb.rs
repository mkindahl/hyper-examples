//! Simple key-value database that you can update through the web
//! interface. It is only intended to demonstrate how to share a state
//! between several futures.
//!
//! Start it using, for example:
//! ```bash
//! cargo run --example kvdb
//! ```

#![feature(async_closure)]

use futures::lock::Mutex;
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Method, Request, Response, Server, StatusCode,
};
use log::info;
use std::{collections::HashMap, sync::Arc};
use url::form_urlencoded;

static MISSING_KEY: &[u8] = b"Missing 'key' field";
static MISSING_VALUE: &[u8] = b"Missing 'value' field";

fn make_row(method: Method, key: Option<&str>, value: Option<&str>) -> String {
    let button = match method {
        Method::POST => "Insert",
        Method::DELETE => "Delete",
        _ => "Unsupported",
    };

    let key_field = match key {
        Some(key) => format!(r#"<input type="hidden", name="key" value="{0}">{0}"#, key),
        None => r#"<input type="text" name="key">"#.to_string(),
    };

    let value_field = match value {
        Some(value) => value,
        None => r#"<input type="text" name="value">"#,
    };

    let cells = vec![
        format!(r#"<td>{}</td>"#, key_field),
        format!(r#"<td>{}</td>"#, value_field),
        format!(r#"<td><input type="submit" value="{}"></td>"#, button),
    ];
    format!(
        r#"<tr><form method="POST">{}<input type="hidden" name="action" value="{}"></form></tr>"#,
        cells.join(""),
        method
    )
}

/// Process a request with the given database. It might update the
/// database.
async fn process(
    database: Arc<Mutex<HashMap<String, String>>>,
    req: Request<Body>,
) -> Result<Response<Body>, hyper::Error> {
    let bytes = hyper::body::to_bytes(req).await?;
    let params = form_urlencoded::parse(bytes.as_ref())
        .into_owned()
        .collect::<HashMap<String, String>>();

    // Forms only support GET and POST according to the standard, so
    // we pick the right method based on the "action" field instead.
    let method = match params.get("action") {
        Some(action) if action == "GET" => Method::GET,
        None => Method::GET,
        Some(action) if action == "PUT" => Method::PUT,
        Some(action) if action == "DELETE" => Method::DELETE,
        Some(action) if action == "POST" => Method::POST,
        Some(_) => {
            return Ok(Response::builder()
                .status(StatusCode::UNPROCESSABLE_ENTITY)
                .body("Incorrect value for parameter 'action'".into())
                .unwrap());
        }
    };

    match method {
        Method::GET => {}
        Method::POST => match (params.get("key"), params.get("value")) {
            (Some(ref key), Some(ref value)) => {
                info!("Adding entry: '{}' := '{}'", key, value);
                database
                    .lock()
                    .await
                    .insert(key.to_string(), value.to_string());
            }
            (None, _) => {
                return Ok(Response::builder()
                    .status(StatusCode::UNPROCESSABLE_ENTITY)
                    .body(MISSING_KEY.into())
                    .unwrap());
            }
            (_, None) => {
                return Ok(Response::builder()
                    .status(StatusCode::UNPROCESSABLE_ENTITY)
                    .body(MISSING_VALUE.into())
                    .unwrap());
            }
        },

        Method::DELETE => match params.get("key") {
            Some(ref key) => {
                info!("Deleting entry with key '{}'", key);
                database.lock().await.remove(&key.to_string());
            }
            None => {
                return Ok(Response::builder()
                    .status(StatusCode::UNPROCESSABLE_ENTITY)
                    .body(MISSING_KEY.into())
                    .unwrap());
            }
        },

        _ => {
            return Ok(Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Body::from("Only supports POST, GET, and DELETE"))
                .unwrap());
        }
    }

    Ok(Response::new(Body::from(format!(
        r#"<html><body><table>{}{}</table></body></html>"#,
        database
            .lock()
            .await
            .iter()
            .map(|(key, value)| { make_row(Method::DELETE, Some(key), Some(value)) })
            .collect::<Vec<String>>()
            .join(""),
        make_row(Method::POST, None, None)
    ))))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    env_logger::init();

    let addr = ([127, 0, 0, 1], 3000).into();
    let database: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    let make_service = make_service_fn(move |_| {
        let database = database.clone();
        async move { Ok::<_, hyper::Error>(service_fn(move |req| process(database.clone(), req))) }
    });
    let server = Server::bind(&addr).serve(make_service);

    println!("Listening on http://{}", addr);

    server.await?;

    Ok(())
}
