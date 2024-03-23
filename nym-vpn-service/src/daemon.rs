use axum::{routing::get, Router};
use log::info;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tower_http::cors::CorsLayer;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[allow(dead_code)]
async fn handler() -> String {
    let timestamp = std::time::UNIX_EPOCH.elapsed().unwrap().as_millis();
    format!("Hello from NymVPN service v{} at {}", VERSION, timestamp)
}

async fn run_http_server() -> anyhow::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:3333").await?;
    let app = Router::new()
        .route("/", get(handler))
        .layer(CorsLayer::permissive());
    axum::serve(listener, app).await.unwrap();
    Ok(())
}

#[derive(Clone, PartialEq, Debug)]
pub enum DaemonState {
    Starting,
    Running,
    Stopping,
    Stopped,
}

#[derive(Clone)]
pub struct Daemon {
    state: Arc<Mutex<DaemonState>>,
    channel: Sender<DaemonState>,
}

impl Daemon {
    pub fn new(tx: Sender<DaemonState>) -> Self {
        Daemon {
            state: Arc::new(Mutex::new(DaemonState::Starting)),
            channel: tx,
        }
    }

    pub async fn get_status(&self) -> DaemonState {
        self.state.lock().await.clone()
    }

    async fn set_status(&self, new_status: DaemonState) {
        let mut guard = self.state.lock().await;
        *guard = new_status.clone();
        self.channel.send(new_status).unwrap();
    }

    pub async fn start(&mut self) {
        self.set_status(DaemonState::Starting).await;

        tokio::spawn(async {
            run_http_server().await.unwrap();
        });

        self.set_status(DaemonState::Running).await;

        let runner_state = self.state.clone();

        tokio::spawn(async move {
            loop {
                let runner_guard = runner_state.lock().await;
                if *runner_guard == DaemonState::Stopped || *runner_guard == DaemonState::Stopping {
                    break;
                }
                info!("Daemon is running");

                sleep(Duration::from_secs(5)).await;
            }
        });
    }

    pub async fn stop(&mut self) {
        self.set_status(DaemonState::Stopping).await;
        sleep(Duration::from_secs(3)).await;
        self.set_status(DaemonState::Stopped).await;
    }
}
