use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    fs,
};
use tower_http::services::ServeDir;

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

#[derive(Debug, Deserialize)]
struct Config {
    admin_token: String,
    port: u16,
    host: String,
}

impl Config {
    fn load() -> Self {
        let content = fs::read_to_string("config.json")
            .expect("Failed to read config.json");
        serde_json::from_str(&content)
            .expect("Failed to parse config.json")
    }
}

type DbState = Arc<RwLock<HashMap<String, UrlEntry>>>;

const DB_FILE: &str = "urls.json";

type AppState = (DbState, String);

#[tokio::main]
async fn main() {
    let config = Config::load();
    let db: DbState = Arc::new(RwLock::new(load_db()));

    let app = Router::new()
        .route("/", get(serve_index))
        .route("/s/:short_url", get(redirect_to_long_url))
        .route("/api/urls", get(list_urls).post(create_url))
        .route("/api/urls/:short_url", get(get_url).delete(delete_url))
        .nest_service("/admin", ServeDir::new("static"))
        .with_state((db.clone(), config.admin_token));

    let addr = format!("{}:{}", config.host, config.port);
    println!("Server running on http://localhost:{}", config.port);
    
    axum::serve(
        tokio::net::TcpListener::bind(&addr).await.unwrap(),
        app
    )
    .await
    .unwrap();
}

fn load_db() -> HashMap<String, UrlEntry> {
    std::fs::read_to_string(DB_FILE)
        .map(|content| serde_json::from_str(&content).unwrap_or_default())
        .unwrap_or_default()
}

fn save_db(db: &HashMap<String, UrlEntry>) {
    if let Ok(content) = serde_json::to_string_pretty(db) {
        let _ = std::fs::write(DB_FILE, content);
    }
}

async fn serve_index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn redirect_to_long_url(
    Path(short_url): Path<String>,
    State((db, _)): State<AppState>,
) -> impl IntoResponse {
    // 先获取长 URL
    let long_url = {
        let db_read = db.read().unwrap();
        db_read.get(&short_url).map(|entry| entry.long_url.clone())
    };
    
    if let Some(long_url) = long_url {
        // 更新访问统计
        let mut db_write = db.write().unwrap();
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

async fn create_url(
    headers: HeaderMap,
    State((db, admin_token)): State<AppState>,
    Json(req): Json<CreateUrlRequest>,
) -> impl IntoResponse {
    if !is_admin(&headers, &admin_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let mut db = db.write().unwrap();
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

async fn list_urls(
    headers: HeaderMap,
    State((db, admin_token)): State<AppState>,
) -> impl IntoResponse {
    if !is_admin(&headers, &admin_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let db = db.read().unwrap();
    Json(db.values().cloned().collect::<Vec<_>>()).into_response()
}

async fn get_url(
    headers: HeaderMap,
    Path(short_url): Path<String>,
    State((db, admin_token)): State<AppState>,
) -> impl IntoResponse {
    if !is_admin(&headers, &admin_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let db = db.read().unwrap();
    if let Some(entry) = db.get(&short_url) {
        Json(entry).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

async fn delete_url(
    headers: HeaderMap,
    Path(short_url): Path<String>,
    State((db, admin_token)): State<AppState>,
) -> impl IntoResponse {
    if !is_admin(&headers, &admin_token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let mut db = db.write().unwrap();
    if db.remove(&short_url).is_some() {
        save_db(&db);
        StatusCode::OK.into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
