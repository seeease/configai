pub mod api;
pub mod core;
pub mod error;
pub mod models;
pub mod storage;

fn main() {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("serve");

    let config_dir = parse_arg(&args, "--config-dir").unwrap_or_else(|| "./config".to_string());
    let port = parse_arg(&args, "--port").unwrap_or_else(|| "3000".to_string());

    match command {
        "init" => init(&config_dir),
        _ => {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(serve(&config_dir, &port));
        }
    }
}

fn parse_arg(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(|s| s.to_string())
}

fn init(config_dir: &str) {
    let base = std::path::Path::new(config_dir);
    std::fs::create_dir_all(base.join("shared")).unwrap();
    std::fs::create_dir_all(base.join("projects/example")).unwrap();

    std::fs::write(
        base.join("shared/default.yaml"),
        "# Shared config (all projects)\nlog_level: info\n",
    )
    .unwrap();

    std::fs::write(
        base.join("projects/example/project.yaml"),
        "description: \"Example project\"\napi_keys:\n  - key: \"change-me-to-a-real-uuid\"\n",
    )
    .unwrap();

    std::fs::write(
        base.join("projects/example/default.yaml"),
        "# Project config\ndb_host: localhost\ndb_port: 5432\n",
    )
    .unwrap();

    println!("Config directory initialized: {}", config_dir);
}

async fn serve(config_dir: &str, port: &str) {
    use notify::{Event, EventKind, RecursiveMode, Watcher};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    let config_path = std::path::PathBuf::from(config_dir);
    let center = match core::ConfigCenter::new(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to initialize: {}", e);
            std::process::exit(1);
        }
    };

    eprintln!("[DEBUG] ConfigCenter loaded from: {}", config_dir);
    eprintln!("[DEBUG] Projects: {:?}", center.list_projects());

    let state: api::AppState = Arc::new(RwLock::new(center));
    let reload_state = state.clone();
    let reload_path = config_path.clone();

    // File watcher - only react to yaml file changes
    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);

    let watch_path = config_path.clone();
    std::thread::spawn(move || {
        let tx = tx;
        let mut watcher =
            notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    if !matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                        return;
                    }
                    // Only trigger for yaml/yml files
                    let is_yaml = event.paths.iter().any(|p| {
                        p.extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e == "yaml" || e == "yml")
                            .unwrap_or(false)
                    });
                    if is_yaml {
                        let _ = tx.blocking_send(());
                    }
                }
            })
            .expect("Failed to create file watcher");

        // Only watch if config dir exists
        if watch_path.exists() {
            watcher
                .watch(&watch_path, RecursiveMode::Recursive)
                .expect("Failed to watch config directory");
        }

        loop {
            std::thread::sleep(std::time::Duration::from_secs(3600));
        }
    });

    // Background reload with debounce
    tokio::spawn(async move {
        while rx.recv().await.is_some() {
            // Debounce: wait 500ms and drain any additional notifications
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            while rx.try_recv().is_ok() {}

            match core::ConfigCenter::new(&reload_path) {
                Ok(new_center) => {
                    let mut center = reload_state.write().await;
                    *center = new_center;
                    tracing::info!("Config reloaded");
                }
                Err(e) => {
                    tracing::warn!("Failed to reload config: {}", e);
                }
            }
        }
    });

    let router = api::create_router(state);
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    eprintln!("[DEBUG] API Server listening on: http://{}", addr);
    axum::serve(listener, router).await.unwrap();
}
