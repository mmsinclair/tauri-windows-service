# Tauri App with Windows Service

This repo contains:

- Windows Service
    - Rust using `windows-service-rs` crate
    - Creates an event log source wired to `rustlog`
    - When running, creates an `axum` HTTP server on http://localhost:3333 that responds with the service binary version and a timestamp
- Tauri App
    - Windows WIX installer extension to install and re-install a Windows Service
    - UI polls http://localhost:3333 every second showing the service version (if running) or failure error information
