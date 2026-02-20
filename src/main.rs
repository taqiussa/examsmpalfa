use axum::{Extension, Router};
use dotenvy::dotenv;
use std::net::SocketAddr;
use tera::Tera;
use tower_http::services::ServeDir;

use crate::utils::functions::TahunOptions;

mod config;
mod controllers;
mod layouts;
mod middlewares;
mod models;
mod routes;
mod utils;

#[cfg(test)]
mod test_support;

#[tokio::main]
async fn main() {
    dotenv().ok();

    // =========================
    // Database
    // =========================
    let db = config::database::connect().await;

    // =========================
    // Tera
    // =========================
    let mut tera = Tera::new("templates/**/*").unwrap_or_else(|e| {
        eprintln!("❌ Tera init error: {}", e);
        std::process::exit(1);
    });

    tera.register_function("tahun_options", TahunOptions);

    println!("📦 Loaded templates:");
    for name in tera.get_template_names() {
        println!(" - {}", name);
    }

    // =========================
    // Router
    // =========================
    let s3_state = config::s3::build_from_env().await;
    let app = Router::new()
        .nest_service("/static", ServeDir::new("static"))
        .merge(routes::web_routes::web_routes())
        .layer(Extension(db))
        .layer(Extension(tera))
        .layer(Extension(s3_state));

    // =========================
    // Port
    // =========================
    let port = std::env::var("APP_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3000);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    println!("🚀 Server running on http://{}", addr);

    // =========================
    // Start server
    // =========================
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| {
            eprintln!("❌ Failed to bind port: {}", e);
            std::process::exit(1);
        });

    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("❌ Server error: {}", e);
    }
}
