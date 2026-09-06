use crate::pty_terminal;
use anyhow::{Context as _, Result};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use tower_http::services::{ServeDir, ServeFile};

pub async fn serve(addr: SocketAddr, rpc_addr: SocketAddr) -> Result<()> {
    let root = resolve_root()?;
    let index = root.join("index.html");
    let config = runtime_config(rpc_addr);
    let web_port = addr.port();
    let app = Router::new()
        .route(
            "/steward-config.js",
            get(move || {
                let config = config.clone();
                async move { ([(header::CONTENT_TYPE, "application/javascript")], config) }
            }),
        )
        .route(
            "/terminal/ws",
            get(move |headers: HeaderMap, ws: WebSocketUpgrade| async move {
                terminal_upgrade(headers, ws, web_port).await
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

#[derive(Deserialize)]
struct ResizeMessage {
    cols: u16,
    rows: u16,
}

async fn terminal_upgrade(headers: HeaderMap, ws: WebSocketUpgrade, web_port: u16) -> Response {
    let trusted = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|origin| is_trusted_web_origin(origin, web_port));
    if !trusted {
        return (StatusCode::FORBIDDEN, "untrusted WebSocket origin").into_response();
    }
    ws.on_upgrade(run_terminal_session).into_response()
}

fn is_trusted_web_origin(origin: &str, expected_port: u16) -> bool {
    let Ok(uri) = origin.parse::<Uri>() else {
        return false;
    };
    if uri.scheme_str() != Some("http") {
        return false;
    }
    let Some(authority) = uri.authority() else {
        return false;
    };
    if authority.port_u16() != Some(expected_port) {
        return false;
    }
    let host = authority.host();
    host.eq_ignore_ascii_case("localhost")
        || host == "127.0.0.1"
        || host == "::1"
        || host == "[::1]"
}

async fn run_terminal_session(mut socket: WebSocket) {
    let (mut pty, mut output_rx, child) = match pty_terminal::spawn_shell(80, 24) {
        Ok(session) => session,
        Err(error) => {
            let _ = socket
                .send(Message::Text(format!(
                    "failed to start native terminal: {error:#}"
                )))
                .await;
            return;
        }
    };

    loop {
        tokio::select! {
            chunk = output_rx.recv() => {
                match chunk {
                    Some(bytes) => {
                        if socket.send(Message::Binary(bytes)).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Binary(data))) => {
                        if pty.write(&data).is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(resize) = serde_json::from_str::<ResizeMessage>(&text) {
                            let _ = pty.resize(resize.cols, resize.rows);
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
    pty_terminal::shutdown(child).await;
}

#[cfg(test)]
mod tests {
    use super::{is_trusted_web_origin, runtime_config, validate_root};
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

    #[test]
    fn browser_origin_must_match_the_local_web_console() {
        assert!(is_trusted_web_origin("http://127.0.0.1:3000", 3000));
        assert!(is_trusted_web_origin("http://localhost:3000", 3000));
        assert!(is_trusted_web_origin("http://[::1]:3000", 3000));

        assert!(!is_trusted_web_origin("https://steward.example.com", 3000));
        assert!(!is_trusted_web_origin("http://127.0.0.1:4000", 3000));
        assert!(!is_trusted_web_origin(
            "http://localhost.evil.example:3000",
            3000
        ));
        assert!(!is_trusted_web_origin("null", 3000));
        assert!(!is_trusted_web_origin("", 3000));
    }
}
