//! Embedded HTTP server for the config UI.
//!
//! Serves a small HTML/JS config editor on `http://127.0.0.1:7878`.
//! The UI is intentionally minimal — no framework, no build step, no npm.
//! Just vanilla JS so the binary stays single-file and self-contained.

use crate::config::Config;
use anyhow::Result;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

pub const DEFAULT_PORT: u16 = 7878;

const INDEX_HTML: &str = include_str!("ui/static/index.html");
const APP_JS: &str = include_str!("ui/static/app.js");
const STYLE_CSS: &str = include_str!("ui/static/style.css");

pub struct Server {
    config_path: PathBuf,
    state: Arc<Mutex<ConfigState>>,
}

pub struct ConfigState {
    pub config: Config,
    pub last_error: Option<String>,
}

impl Server {
    pub fn new(config_path: PathBuf) -> Result<Self> {
        let config = Config::load(&config_path)?;
        Ok(Self {
            config_path,
            state: Arc::new(Mutex::new(ConfigState {
                config,
                last_error: None,
            })),
        })
    }

    pub fn state(&self) -> Arc<Mutex<ConfigState>> {
        self.state.clone()
    }

    /// Start serving on 127.0.0.1:port. Blocks the calling thread.
    pub fn serve(self, port: u16) -> Result<()> {
        let addr = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(&addr)
            .map_err(|e| anyhow::anyhow!("Failed to bind {}: {}", addr, e))?;
        crate::log::info(&format!("Web UI listening on http://{}", addr));

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let state = self.state.clone();
                    let path = self.config_path.clone();
                    thread::spawn(move || {
                        if let Err(e) = handle_client(stream, state, path) {
                            crate::log::warn(&format!("HTTP client error: {}", e));
                        }
                    });
                }
                Err(e) => {
                    crate::log::warn(&format!("HTTP accept error: {}", e));
                }
            }
        }
        Ok(())
    }
}

fn handle_client(
    mut stream: TcpStream,
    state: Arc<Mutex<ConfigState>>,
    config_path: PathBuf,
) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/");

    // Drain headers (we don't need them for this tiny server)
    loop {
        let mut header = String::new();
        let n = reader.read_line(&mut header)?;
        if n == 0 || header == "\r\n" || header == "\n" {
            break;
        }
    }

    match (method, path) {
        ("GET", "/") => respond(&mut stream, 200, "text/html", INDEX_HTML)?,
        ("GET", "/app.js") => respond(&mut stream, 200, "application/javascript", APP_JS)?,
        ("GET", "/style.css") => respond(&mut stream, 200, "text/css", STYLE_CSS)?,
        ("GET", "/api/config") => {
            let guard = state.lock().unwrap();
            let body = serde_json::to_string_pretty(&guard.config)?;
            respond_json(&mut stream, 200, &body)?;
        }
        ("GET", "/api/status") => {
            let body = format!(
                r#"{{"ok":true,"version":"{}"}}"#,
                env!("CARGO_PKG_VERSION")
            );
            respond_json(&mut stream, 200, &body)?;
        }
        ("POST", "/api/config") => {
            let mut body = String::new();
            reader.read_to_string(&mut body)?;
            match serde_json::from_str::<Config>(&body) {
                Ok(new_cfg) => {
                    if let Err(e) = crate::config::resolve(&new_cfg) {
                        let resp = format!(
                            r#"{{"ok":false,"error":"{}"}}"#,
                            e.to_string().replace('"', "\\\"")
                        );
                        respond_json(&mut stream, 400, &resp)?;
                    } else if let Err(e) = new_cfg.save(&config_path) {
                        let resp = format!(
                            r#"{{"ok":false,"error":"{}"}}"#,
                            e.to_string().replace('"', "\\\"")
                        );
                        respond_json(&mut stream, 500, &resp)?;
                    } else {
                        let mut guard = state.lock().unwrap();
                        guard.config = new_cfg;
                        guard.last_error = None;
                        respond_json(&mut stream, 200, r#"{"ok":true}"#)?;
                    }
                }
                Err(e) => {
                    let resp = format!(
                        r#"{{"ok":false,"error":"JSON parse: {}"}}"#,
                        e.to_string().replace('"', "\\\"")
                    );
                    respond_json(&mut stream, 400, &resp)?;
                }
            }
        }
        _ => respond(&mut stream, 404, "text/plain", "Not Found")?,
    }

    Ok(())
}

fn respond(stream: &mut TcpStream, code: u16, content_type: &str, body: &str) -> Result<()> {
    let status = match code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    };
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{}",
        code, status, content_type, body.len(), body
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

fn respond_json(stream: &mut TcpStream, code: u16, body: &str) -> Result<()> {
    respond(stream, code, "application/json", body)
}
