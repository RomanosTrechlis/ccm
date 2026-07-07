//! ccm — Claude Code usage monitor. Wraps the ccusage CLI; never parses
//! ~/.claude itself. Setup, polling tasks, tray, and commands live here.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ccusage;
mod gauge;

use chrono::{Local, Utc};
use gauge::{Band, GaugeState};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Mutex;
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_store::StoreExt;
use tokio::sync::Notify;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Settings {
    /// user-set 5-hour token budget — the gauge ceiling. No plan-limit API
    /// exists, so this is an estimate the user calibrates from past blocks.
    token_budget: u64,
    blocks_poll_secs: u64,
    summary_poll_secs: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self { token_budget: 50_000_000, blocks_poll_secs: 7, summary_poll_secs: 45 }
    }
}

impl Settings {
    fn clamped(mut self) -> Self {
        self.blocks_poll_secs = self.blocks_poll_secs.clamp(5, 10);
        self.summary_poll_secs = self.summary_poll_secs.clamp(30, 60);
        if self.token_budget == 0 {
            self.token_budget = Settings::default().token_budget;
        }
        self
    }
}

#[derive(Default)]
struct Cache {
    gauge: GaugeState,
    block: Option<Value>,
    daily: Option<Value>,
    sessions: Option<Value>,
    cost_today: f64,
    error: Option<String>,
}

#[derive(Default)]
struct AppState {
    settings: Mutex<Settings>,
    runner: Mutex<Option<(ccusage::Runner, String)>>,
    cache: Mutex<Cache>,
    refresh_fast: Notify,
    refresh_slow: Notify,
}

// ---------------------------------------------------------------- polling

async fn ensure_runner(app: &AppHandle) -> Result<ccusage::Runner, String> {
    if let Some((r, _)) = *app.state::<AppState>().runner.lock().unwrap() {
        return Ok(r);
    }
    let (r, version) = ccusage::detect().await?;
    *app.state::<AppState>().runner.lock().unwrap() = Some((r, version));
    Ok(r)
}

fn report_error(app: &AppHandle, message: String) {
    app.state::<AppState>().cache.lock().unwrap().error = Some(message.clone());
    let _ = app.emit("ccm://error", serde_json::json!({ "message": message }));
    update_tray(app);
}

/// Recompute GaugeState from the cached block + today's cost, then push it to
/// the UI and the tray — the single source of truth for both.
fn recompute_gauge(app: &AppHandle) {
    let state = app.state::<AppState>();
    let budget = state.settings.lock().unwrap().token_budget;
    let (block_val, cost_today) = {
        let c = state.cache.lock().unwrap();
        (c.block.clone(), c.cost_today)
    };
    let block: Option<gauge::Block> =
        block_val.as_ref().and_then(|v| serde_json::from_value(v.clone()).ok());
    let g = gauge::compute(block.as_ref(), budget, cost_today, Utc::now());
    state.cache.lock().unwrap().gauge = g.clone();
    let _ = app.emit("ccm://active-block", serde_json::json!({ "gauge": g, "block": block_val }));
    update_tray(app);
}

async fn poll_fast(app: &AppHandle) {
    let result = match ensure_runner(app).await {
        Ok(r) => ccusage::run_json(r, &["claude", "blocks", "--active", "--json"]).await,
        Err(e) => Err(e),
    };
    match result {
        Ok(v) => {
            let active = v
                .get("blocks")
                .and_then(Value::as_array)
                .and_then(|a| {
                    a.iter().find(|b| b.get("isActive").and_then(Value::as_bool).unwrap_or(false))
                })
                .cloned();
            {
                let state = app.state::<AppState>();
                let mut c = state.cache.lock().unwrap();
                c.block = active;
                c.error = None;
            }
            recompute_gauge(app);
        }
        Err(e) => report_error(app, e),
    }
}

async fn poll_slow(app: &AppHandle) {
    let runner = match ensure_runner(app).await {
        Ok(r) => r,
        Err(e) => return report_error(app, e),
    };
    let daily = ccusage::run_json(runner, &["claude", "daily", "--json"]).await;
    let sessions = ccusage::run_json(runner, &["claude", "session", "--json"]).await;
    match (daily, sessions) {
        (Ok(d), Ok(s)) => {
            let today = Local::now().format("%Y-%m-%d").to_string();
            let cost_today = d
                .get("daily")
                .and_then(Value::as_array)
                .and_then(|rows| {
                    rows.iter().find(|r| r.get("date").and_then(Value::as_str) == Some(&today))
                })
                .and_then(|r| r.get("totalCost"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            {
                let state = app.state::<AppState>();
                let mut c = state.cache.lock().unwrap();
                c.daily = Some(d.clone());
                c.sessions = Some(s.clone());
                c.cost_today = cost_today;
                c.error = None;
            }
            let _ = app.emit("ccm://usage", serde_json::json!({ "daily": d, "sessions": s }));
            recompute_gauge(app); // tray tooltip shows "$X today"
        }
        (Err(e), _) | (_, Err(e)) => report_error(app, e),
    }
}

async fn poll_loop(app: AppHandle, fast: bool) {
    loop {
        if fast {
            poll_fast(&app).await;
        } else {
            poll_slow(&app).await;
        }
        let state = app.state::<AppState>();
        let secs = {
            let s = state.settings.lock().unwrap();
            if fast { s.blocks_poll_secs } else { s.summary_poll_secs }
        };
        let notified =
            if fast { state.refresh_fast.notified() } else { state.refresh_slow.notified() };
        // sleep the interval, but wake early on refresh_now
        let _ = tokio::time::timeout(Duration::from_secs(secs), notified).await;
    }
}

// ------------------------------------------------------------------- tray

fn tray_icon_for(band: Band) -> Image<'static> {
    // prebuilt icons (see icons/gen-icons.mjs) — the color band is the signal
    let bytes: &[u8] = match band {
        Band::Green => include_bytes!("../icons/tray-green.png"),
        Band::Amber => include_bytes!("../icons/tray-amber.png"),
        Band::Red => include_bytes!("../icons/tray-red.png"),
        Band::Grey => include_bytes!("../icons/tray-grey.png"),
    };
    Image::from_bytes(bytes).expect("bundled tray icon is valid PNG")
}

fn update_tray(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (gauge, error) = {
        let c = state.cache.lock().unwrap();
        (c.gauge.clone(), c.error.clone())
    };
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_icon(Some(tray_icon_for(gauge::color_band(&gauge, error.is_some()))));
        let _ = tray.set_tooltip(Some(gauge::tooltip(&gauge, error.as_deref())));
    }
}

fn toggle_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
        } else {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn refresh_all(app: &AppHandle) {
    let state = app.state::<AppState>();
    state.refresh_fast.notify_waiters();
    state.refresh_slow.notify_waiters();
}

// --------------------------------------------------------------- commands

#[tauri::command]
fn get_settings(state: tauri::State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn set_settings(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    let s = settings.clamped();
    *app.state::<AppState>().settings.lock().unwrap() = s.clone();
    let store = app.store("settings.json").map_err(|e| e.to_string())?;
    store.set("settings", serde_json::to_value(&s).map_err(|e| e.to_string())?);
    store.save().map_err(|e| e.to_string())?;
    recompute_gauge(&app); // budget change moves the gauge + tray immediately
    Ok(s)
}

#[tauri::command]
fn refresh_now(app: AppHandle) {
    refresh_all(&app);
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CcusageStatus {
    found: bool,
    version: Option<String>,
    source: Option<String>,
    pinned: String,
    error: Option<String>,
}

#[tauri::command]
async fn ccusage_status(app: AppHandle) -> CcusageStatus {
    let cached = app.state::<AppState>().runner.lock().unwrap().clone();
    let result = match cached {
        Some(rv) => Ok(rv),
        None => ccusage::detect().await.inspect(|rv| {
            *app.state::<AppState>().runner.lock().unwrap() = Some(rv.clone());
        }),
    };
    match result {
        Ok((r, version)) => CcusageStatus {
            found: true,
            version: Some(version),
            source: Some(r.describe()),
            pinned: ccusage::PINNED_VERSION.into(),
            error: None,
        },
        Err(e) => CcusageStatus {
            found: false,
            version: None,
            source: None,
            pinned: ccusage::PINNED_VERSION.into(),
            error: Some(e),
        },
    }
}

#[tauri::command]
fn get_snapshot(state: tauri::State<'_, AppState>) -> Value {
    let c = state.cache.lock().unwrap();
    serde_json::json!({
        "gauge": c.gauge,
        "block": c.block,
        "daily": c.daily,
        "sessions": c.sessions,
        "error": c.error,
    })
}

// ------------------------------------------------------------------ setup

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_settings,
            refresh_now,
            ccusage_status,
            get_snapshot
        ])
        .on_window_event(|window, event| {
            // close-to-tray; Quit lives in the tray menu
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .setup(|app| {
            // load persisted settings
            let store = app.store("settings.json")?;
            if let Some(v) = store.get("settings") {
                if let Ok(s) = serde_json::from_value::<Settings>(v) {
                    *app.state::<AppState>().settings.lock().unwrap() = s.clamped();
                }
            }

            // tray: color-coded icon + tooltip, left-click toggles the window
            let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
            let refresh = MenuItem::with_id(app, "refresh", "Refresh", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &refresh, &quit])?;
            TrayIconBuilder::with_id("main")
                .icon(tray_icon_for(Band::Grey))
                .tooltip("ccm — starting…")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_window(app),
                    "refresh" => refresh_all(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_window(tray.app_handle());
                    }
                })
                .build(app)?;

            // two timers: fast for the active block, slow for daily + sessions
            tauri::async_runtime::spawn(poll_loop(app.handle().clone(), true));
            tauri::async_runtime::spawn(poll_loop(app.handle().clone(), false));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running ccm");
}
