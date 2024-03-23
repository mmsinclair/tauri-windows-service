# Tauri App with Windows Service

This repo contains:

- Windows Service
    - Rust using `windows-service-rs` crate
    - Creates an event log source wired to `rustlog`
    - When running, creates an `axum` HTTP server on http://localhost:3333 that responds with the service binary version and a timestamp
- Tauri App
    - Windows WIX installer extension to install and re-install a Windows Service
    - UI polls http://localhost:3333 every second showing the service version (if running) or failure error information


## Building the installer

First build the service from `nym-vpn-service`:

```
cargo build --release
```

From `nym-vpn-app` run:

```
npm install
npm run build:release
```

The **MSI installer** can be found in: ``.

NB: Don't use the NSIS installer, because that doesn't have customisations.
