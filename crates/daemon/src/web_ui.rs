use anyhow::{Context as _, Result};
use axum::http::header;
use axum::routing::get;
use axum::Router;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use tower_http::services::{ServeDir, ServeFile};

pub async fn serve(addr: SocketAddr, rpc_addr: SocketAddr) -> Result<()> {
    let root = resolve_root()?;
    let index = root.join("index.html");
    let config = runtime_config(rpc_addr);
    let app = Router::new()
        .route(
            "/steward-config.js",
            get(move || {
                let config = config.clone();
                async move { ([(header::CONTENT_TYPE, "application/javascript")], config) }
            }),
        )
        .fallback_service(ServeDir::new(root).fallback(ServeFile::new(index)));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding Steward Web UI to {addr}"))?;
    tracing::info!(%addr, "Steward Web UI listening");
    axum::serve(listener, app)
        .await
        .context("serving Steward Web UI")
}

fn runtime_config(rpc_addr: SocketAddr) -> String {
    format!("window.__STEWARD_RPC_URL__ = \"http://{rpc_addr}\";")
}

fn resolve_root() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("STEWARD_WEB_ROOT") {
        return validate_root(PathBuf::from(path));
    }
    let executable = std::env::current_exe().context("resolving Steward daemon executable")?;
    if let Some(parent) = executable.parent() {
        let installed = parent.join("web-ui");
        if installed.join("index.html").is_file() {
            return Ok(installed);
        }
    }
    validate_root(PathBuf::from("web-ui").join("out"))
}

fn validate_root(path: PathBuf) -> Result<PathBuf> {
    if !Path::new(&path).join("index.html").is_file() {
        anyhow::bail!(
            "Steward Web UI assets were not found at {}; reinstall Steward or set STEWARD_WEB_ROOT",
            path.display()
        );
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::{runtime_config, validate_root};
    use std::net::SocketAddr;

    #[test]
    fn web_root_requires_exported_index() {
        let temp = tempfile::tempdir().expect("temporary directory");
        assert!(validate_root(temp.path().to_path_buf()).is_err());
        std::fs::write(temp.path().join("index.html"), "steward").expect("write index");
        assert_eq!(
            validate_root(temp.path().to_path_buf()).expect("valid root"),
            temp.path()
        );
    }

    #[test]
    fn runtime_config_uses_the_actual_rpc_address() {
        let address = SocketAddr::from(([127, 0, 0, 1], 52123));
        assert_eq!(
            runtime_config(address),
            "window.__STEWARD_RPC_URL__ = \"http://127.0.0.1:52123\";"
        );
    }
}
