use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use axum_macros::FromRef;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use tower_http::services::ServeDir;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Config {
    admin_token: String,
    port: u16,
    host: String,
    api_prefix: String,
    redirect_prefix: String,
}

impl Config {
    fn load() -> Self {
        let content = std::fs::read_to_string("config.json")
            .expect("Failed to read config.json");
        serde_json::from_str(&content)
            .expect("Failed to parse config.json")
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct UrlEntry {
    short_url: String,
    long_url: String,
    note: String,
    visit_count: u32,
    last_visit: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateUrlRequest {
    long_url: String,
    note: String,
}

type DbState = Arc<RwLock<HashMap<String, UrlEntry>>>;

#[derive(Clone, FromRef)]
struct AppState {
    db: DbState,
    admin_token: String,
    config: Config,
}

#[tokio::main]
async fn main() {
    let config = Config::load();
    let db: DbState = Arc::new(RwLock::new(load_db()));
    
    generate_frontend_config(&config);

    let state = AppState {
        db: db.clone(),
        admin_token: config.admin_token.clone(),
        config: config.clone(),
    };

    let app = Router::new()
        .route("/", get(serve_index))
        .route(&format!("{}/urls", config.api_prefix), get(list_urls).post(create_url))
        .route(&format!("{}/urls/:short_url", config.api_prefix), get(get_url).delete(delete_url))
        .route(&format!("{}/:short_url", config.redirect_prefix), get(redirect_to_long_url))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    println!("Server running on http://{}:{}", config.host, config.port);
    
    axum::serve(
        tokio::net::TcpListener::bind(format!("{}:{}", config.host, config.port))
            .await
            .unwrap(),
        app
    )
    .await
    .unwrap();
}

async fn list_urls(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Response {
    if !is_admin(&headers, &state.admin_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let db = state.db.read().unwrap();
    Json(db.values().cloned().collect::<Vec<_>>()).into_response()
}

async fn create_url(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<CreateUrlRequest>,
) -> Response {
    if !is_admin(&headers, &state.admin_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let mut db = state.db.write().unwrap();
    let short_url = generate_short_url();
    
    let entry = UrlEntry {
        short_url: short_url.clone(),
        long_url: req.long_url,
        note: req.note,
        visit_count: 0,
        last_visit: None,
    };
    
    db.insert(short_url.clone(), entry.clone());
    save_db(&db);
    
    Json(entry).into_response()
}

async fn get_url(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(short_url): Path<String>,
) -> Response {
    if !is_admin(&headers, &state.admin_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let db = state.db.read().unwrap();
    if let Some(entry) = db.get(&short_url) {
        Json(entry).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

async fn delete_url(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(short_url): Path<String>,
) -> Response {
    if !is_admin(&headers, &state.admin_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let mut db = state.db.write().unwrap();
    if db.remove(&short_url).is_some() {
        save_db(&db);
        StatusCode::OK.into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

async fn redirect_to_long_url(
    Path(short_url): Path<String>,
    State(state): State<AppState>,
) -> Response {
    let long_url = {
        let db_read = state.db.read().unwrap();
        db_read.get(&short_url).map(|entry| entry.long_url.clone())
    };
    
    if let Some(long_url) = long_url {
        let mut db_write = state.db.write().unwrap();
        if let Some(entry) = db_write.get_mut(&short_url) {
            entry.visit_count += 1;
            entry.last_visit = Some(Utc::now());
            save_db(&db_write);
        }
        Redirect::permanent(&long_url).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

async fn serve_index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

fn generate_frontend_config(config: &Config) {
    let template = std::fs::read_to_string("static/config.template.js")
        .expect("Failed to read config template");
    
    let config_js = template
        .replace("{{API_PREFIX}}", &config.api_prefix)
        .replace("{{REDIRECT_PREFIX}}", &config.redirect_prefix);
    
    std::fs::write("static/config.js", config_js)
        .expect("Failed to write config.js");
}

fn load_db() -> HashMap<String, UrlEntry> {
    std::fs::read_to_string("urls.json")
        .map(|content| serde_json::from_str(&content).unwrap_or_default())
        .unwrap_or_default()
}

fn save_db(db: &HashMap<String, UrlEntry>) {
    if let Ok(content) = serde_json::to_string_pretty(db) {
        let _ = std::fs::write("urls.json", content);
    }
}

fn generate_short_url() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let num: u32 = rng.gen_range(0..1000000);
    format!("{:06}", num)
}

fn is_admin(headers: &HeaderMap, admin_token: &str) -> bool {
    headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| v == admin_token)
        .unwrap_or(false)
}
