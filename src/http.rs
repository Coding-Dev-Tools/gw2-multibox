//! Embedded HTTP server for the config UI.
//!
//! Serves a small HTML/JS config editor on `http://127.0.0.1:7878`.
//! The UI is intentionally minimal — no framework, no build step, no npm.
//! Just vanilla JS so the binary stays single-file and self-contained.

use crate::config::{Config, gw2_template};
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
            let body = format!(r#"{{"ok":true,"version":"{}"}}"#, env!("CARGO_PKG_VERSION"));
            respond_json(&mut stream, 200, &body)?;
        }
        ("GET", "/api/monitors") => {
            let monitors = crate::window::list_monitors();
            let body = serde_json::to_string(&monitors)?;
            respond_json(&mut stream, 200, &body)?;
        }
        ("GET", "/api/windows") => {
            let windows_raw = crate::window::list_all_windows_with_rect();
            let windows: Vec<_> = windows_raw.iter().map(|(w, r)| (w.to_json(), r)).collect();
            let body = serde_json::to_string(&windows)?;
            respond_json(&mut stream, 200, &body)?;
        }
        ("POST", "/api/wizard/create") => {
            let mut body = String::new();
            reader.read_to_string(&mut body)?;
            #[derive(serde::Deserialize)]
            struct WizardReq {
                game: String,
                account_count: usize,
                layout: String,
            }
            match serde_json::from_str::<WizardReq>(&body) {
                Ok(req) => match build_wizard_config(&req.game, req.account_count, &req.layout) {
                    Ok(new_cfg) => {
                        // Save and respond
                        if let Err(e) = crate::config::resolve(&new_cfg) {
                            let resp = error_body(&e.to_string());
                            respond_json(&mut stream, 400, &resp)?;
                        } else if let Err(e) = new_cfg.save(&config_path) {
                            let resp = error_body(&e.to_string());
                            respond_json(&mut stream, 500, &resp)?;
                        } else {
                            let mut guard = state.lock().unwrap();
                            guard.config = new_cfg.clone();
                            guard.last_error = None;
                            let body = serde_json::to_string(
                                &serde_json::json!({"ok": true, "config": new_cfg}),
                            )?;
                            respond_json(&mut stream, 200, &body)?;
                        }
                    }
                    Err(msg) => {
                        let resp = error_body(&msg);
                        respond_json(&mut stream, 400, &resp)?;
                    }
                },
                Err(e) => {
                    let resp = error_body(&e.to_string());
                    respond_json(&mut stream, 400, &resp)?;
                }
            }
        }
        ("POST", "/api/config") => {
            let mut body = String::new();
            reader.read_to_string(&mut body)?;
            match serde_json::from_str::<Config>(&body) {
                Ok(new_cfg) => {
                    if let Err(e) = crate::config::resolve(&new_cfg) {
                        let resp = error_body(&e.to_string());
                        respond_json(&mut stream, 400, &resp)?;
                    } else if let Err(e) = new_cfg.save(&config_path) {
                        let resp = error_body(&e.to_string());
                        respond_json(&mut stream, 500, &resp)?;
                    } else {
                        let mut guard = state.lock().unwrap();
                        guard.config = new_cfg;
                        guard.last_error = None;
                        respond_json(&mut stream, 200, r#"{"ok":true}"#)?;
                    }
                }
                Err(e) => {
                    let resp = error_body(&e.to_string());
                    respond_json(&mut stream, 400, &resp)?;
                }
            }
        }
        _ => respond(&mut stream, 404, "text/plain", "Not Found")?,
    }

    Ok(())
}

/// Build a starter config from wizard parameters. Pure so the slot/layout
/// arithmetic is unit-testable without spawning HTTP handlers or Windows APIs.
///
/// Fixes two wizard bugs:
/// - grid layouts used to always emit 4 team slots, so any request with
///   fewer than 4 accounts failed `resolve()` with "unknown account";
///   grid cell count now clamps to `account_count`.
/// - unknown layout names were silently ignored (the game template's
///   default layout shipped unmodified); they are now rejected with 400.
pub(crate) fn build_wizard_config(
    game: &str,
    account_count: usize,
    layout: &str,
) -> std::result::Result<Config, String> {
    const MAX_ACCOUNTS: usize = 16;
    if account_count == 0 {
        return Err("account_count must be at least 1".to_string());
    }
    if account_count > MAX_ACCOUNTS {
        return Err(format!(
            "account_count must be at most {} (got {})",
            MAX_ACCOUNTS, account_count
        ));
    }

    let mut cfg = match game {
        "gw2" => gw2_template(),
        "wow" => crate::config::wow_template(),
        "ffxiv" => crate::config::ffxiv_template(),
        "eve" => crate::config::eve_template(),
        _ => Config::template(),
    };

    cfg.accounts = (1..=account_count)
        .map(|i| crate::config::Account {
            name: format!("Account{}", i),
            game_profile: cfg.game_profiles[0].name.clone(),
            extra_args: None,
        })
        .collect();

    let make_slot = |i: usize, region: &str| crate::config::Slot {
        index: i,
        account: format!("Account{}", i),
        region: region.to_string(),
    };

    match layout {
        "single" => {
            cfg.layout = crate::config::Layout {
                name: "single".to_string(),
                regions: vec![crate::config::Region {
                    name: "fullscreen".to_string(),
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                }],
            };
            cfg.team.slots = vec![make_slot(1, "fullscreen")];
        }
        "grid1x4" | "grid4x1" => {
            let base = cfg
                .layout
                .regions
                .first()
                .ok_or_else(|| "game template has no layout regions".to_string())?
                .clone();
            let horizontal = layout == "grid1x4";
            let n = account_count;
            cfg.layout.regions = (0..n)
                .map(|i| {
                    let (x, y, w, h) = if horizontal {
                        (
                            ((i * base.width as usize) / n) as i32,
                            0,
                            (base.width as usize / n).max(1) as i32,
                            base.height,
                        )
                    } else {
                        (
                            0,
                            ((i * base.height as usize) / n) as i32,
                            base.width,
                            (base.height as usize / n).max(1) as i32,
                        )
                    };
                    crate::config::Region {
                        name: format!("r{}", i + 1),
                        x,
                        y,
                        width: w,
                        height: h,
                    }
                })
                .collect();
            cfg.team.slots = (0..n)
                .map(|i| make_slot(i + 1, &format!("r{}", i + 1)))
                .collect();
        }
        other => {
            return Err(format!(
                "unknown layout '{}' (expected single, grid1x4 or grid4x1)",
                other
            ));
        }
    }
    Ok(cfg)
}

/// Build a JSON error-response body, properly escaping `msg` so that
/// backslashes (common in Windows exe paths such as `C:\Games\...`) and
/// quotes do not produce malformed JSON that the config UI cannot parse.
/// The old manual `format!` only escaped `"`, which emitted invalid JSON
/// (e.g. `\G` from a path) and broke `JSON.parse` in the web UI.
fn error_body(msg: &str) -> String {
    serde_json::json!({ "ok": false, "error": msg }).to_string()
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
        code,
        status,
        content_type,
        body.len(),
        body
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

fn respond_json(stream: &mut TcpStream, code: u16, body: &str) -> Result<()> {
    respond(stream, code, "application/json", body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_body_emits_valid_json_with_backslash_and_quote() {
        // A typical Windows validation error carries backslashes (exe paths)
        // and may carry quotes (profile names). The old manual formatter
        // produced invalid JSON for these, breaking JSON.parse in the UI.
        let msg =
            r#"Game profile 'x': exe_path does not exist: C:\Games\MyGame\game.exe and "quoted""#;
        let body = error_body(msg);
        let v: serde_json::Value =
            serde_json::from_str(&body).expect("error_body must emit valid JSON");
        assert_eq!(v["ok"], serde_json::json!(false));
        assert_eq!(v["error"], serde_json::json!(msg));
    }

    #[test]
    fn error_body_round_trips_message_exactly() {
        let msg = "Slot 2 references unknown account 'Acct\"X'";
        let v: serde_json::Value = serde_json::from_str(&error_body(msg)).unwrap();
        assert_eq!(v["error"].as_str().unwrap(), msg);
    }

    #[test]
    fn wizard_grid_clamps_slots_to_account_count() {
        // Regression: grid layouts used to emit 4 slots even for 2 accounts,
        // making resolve() fail with "unknown account".
        for layout in ["grid1x4", "grid4x1"] {
            let cfg = build_wizard_config("gw2", 2, layout).expect("wizard config");
            assert_eq!(cfg.accounts.len(), 2);
            assert_eq!(cfg.team.slots.len(), 2);
            assert_eq!(cfg.layout.regions.len(), 2);
            crate::config::resolve(&cfg).expect("clamped wizard config must validate");
        }
    }

    #[test]
    fn wizard_single_layout_validates_for_one_account() {
        let cfg = build_wizard_config("gw2", 1, "single").expect("wizard config");
        assert_eq!(cfg.team.slots.len(), 1);
        crate::config::resolve(&cfg).expect("single wizard config must validate");
    }

    #[test]
    fn wizard_rejects_zero_and_oversized_account_counts() {
        assert!(build_wizard_config("gw2", 0, "single").is_err());
        assert!(build_wizard_config("gw2", 17, "single").is_err());
    }

    #[test]
    fn wizard_rejects_unknown_layout() {
        // Regression: unknown layouts were silently ignored.
        let err = build_wizard_config("gw2", 3, "diagonal").unwrap_err();
        assert!(err.contains("unknown layout"), "got: {}", err);
    }

    #[test]
    fn all_known_game_templates_build_valid_wizard_configs() {
        for game in ["gw2", "wow", "ffxiv", "eve", "mystery-game"] {
            let cfg = build_wizard_config(game, 3, "grid1x4")
                .unwrap_or_else(|e| panic!("{}: {}", game, e));
            crate::config::resolve(&cfg).unwrap_or_else(|e| panic!("{}: {}", game, e));
        }
    }
}
