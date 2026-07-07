# Build Spec — ccm (Claude Code Monitor)

A **local, personal** desktop dashboard for your own Claude Code usage — live 5-hour-block burn rate, cost, and a glanceable tray gauge. Built for yourself first; free portfolio/maybe-product second.

## Goal (one sentence)
A **Tauri + SvelteKit** Windows app that visualizes **`ccusage`'s JSON output** as a live dashboard + system-tray gauge — so you can see how close you are to your 5-hour limit without opening a terminal.

## The one architectural decision (read first)
**Wrap `ccusage`; do NOT parse Claude Code's files yourself.**
`ccusage` already does the fragile, high-maintenance work — parsing the undocumented `~/.claude/projects/**/*.jsonl`, maintaining the per-model **pricing table**, and inferring the **5-hour billing blocks**. It emits clean JSON. We consume that JSON and build only the UI.
- ponytail: this offloads the entire maintenance burden (format drift, price changes, block math) onto a 16.5k-star maintained tool. Every fragile part of the earlier "don't build this" verdict becomes *ccusage's* problem, not ours. **Pin a ccusage version** so the schema is stable; bump deliberately.

---

## Stack & constraints
- **Shell:** Tauri v2 (**Rust** core) — native file access, a real `.exe`, a system tray, tiny binary.
- **Backend:** **Rust** (`tokio`, `serde`, `chrono`, `tauri-plugin-store`) owns spawning ccusage, the polling loops, the tray, and the burn-rate math — see the [Rust / Tauri layer](#rust--tauri-layer-the-backend) section.
- **Frontend:** **SvelteKit** (your existing stack) as the Tauri web frontend. Charts via a lightweight lib (e.g. `layerchart`/`chart.js`) — inline, no CDN.
- **Data engine:** `ccusage` (pinned), invoked by the Rust side.
- **Local-only.** No backend we host, no accounts, no network calls of our own. Single user = you.
- **Windows-first.** Tauri gives mac/Linux later for near-free; don't spend on it in v1.

## Architecture / data flow
```
Claude Code ──writes──▶ ~/.claude/projects/**/*.jsonl
                              │
                        ccusage <cmd> --json   (parsing + pricing + block inference)
                              │
        Tauri (Rust): spawn ccusage on a timer ──emit event──▶ SvelteKit dashboard
                              │                                   │
                              └──────────── updates ─────────────▶ Tray icon (color + tooltip)
```
ponytail: Rust polls ccusage and pushes results to the UI via Tauri events. No IPC cleverness, no streaming parse of ccusage's live TUI — just run the `--json` command on an interval.

---

## Data contract (ccusage commands we depend on)
Treat **ccusage's JSON schema as the source of truth** — run each once and pin the shape. Expected commands:

| Command | Gives | Used by |
|---|---|---|
| `ccusage blocks --active --json` | current 5-hour billing block: tokens (in/out/cache), cost, start/elapsed | **Live gauge** (primary) |
| `ccusage daily --json --since <YYYYMMDD>` | per-day tokens by type + cost, by model | Today/week totals, trends |
| `ccusage session --json` | per-session tokens, cost, project, model | Sessions view |
| `ccusage blocks --json` | all 5-hour blocks (history) | Block history |

Each row typically carries: `inputTokens, outputTokens, cacheCreationTokens, cacheReadTokens, totalTokens, costUSD, model(s), date/timestamp`. **Verify exact field names against the pinned ccusage version** and adapt.

### The limit gauge needs a ceiling you provide
There is **no official API for Pro/Max plan-limit consumption** — ccusage infers block *usage* but doesn't know your plan's ceiling. So:
- **Settings holds a user-set "5-hour token budget"** (your estimate of your plan's window ceiling), or use ccusage's `--token-limit max` (historical-max heuristic) as a default.
- Gauge % = `activeBlock.totalTokens / yourBudget`. Forecast ETA = `(budget − used) / burnRate`, where `burnRate = used / elapsedMinutes`.
- ponytail: label the gauge "estimate" in the UI. It's the best obtainable without an API; don't pretend it's exact.

## Refresh loop
- **Active block:** poll `blocks --active --json` every **5–10s**.
- **Daily / session / breakdown:** poll every **30–60s**.
- ponytail: two intervals — fast for the live gauge, slow for the rest. Don't hammer ccusage every second; nothing changes that fast.

---

## Rust / Tauri layer (the backend)
Everything data-side lives in `src-tauri/`. Concretely:

**Crates**
- `tauri` v2 (tray-icon feature) — window, tray, events, `#[tauri::command]`.
- `tokio` — async polling loops (`tokio::time::interval`) + `tokio::process::Command`.
- `serde` + `serde_json` — deserialize ccusage JSON into typed structs.
- `chrono` — block elapsed / burn-rate / ETA math.
- `tauri-plugin-store` — persist settings (token budget, intervals) to a JSON file. ponytail: a store file, not a DB.

**Spawning ccusage (Windows-safe)**
- Resolve the binary once at startup: try `ccusage` on PATH; else `npx` (Windows: `npx.cmd`, or `cmd /C npx ccusage@<PINNED> …`). Cache which one worked.
- Run e.g. `ccusage blocks --active --json`; capture stdout; `serde_json::from_slice` into a struct.
- On non-zero exit / spawn error: emit `ccm://error` with a message; tray goes grey. **Never panic.**
- ponytail: shell out to the pinned CLI; never reimplement or link ccusage internals.

**Polling tasks** (spawned in `tauri::Builder::setup`)
- Task A — every 5–10s → `blocks --active` → compute `GaugeState` → `app.emit("ccm://active-block", …)` + refresh tray.
- Task B — every 30–60s → `daily` + `session` → `app.emit("ccm://usage", …)`.
- Intervals come from the store; the `refresh_now` command triggers both immediately.

**Typed structs** (mirror ccusage JSON — verify field names against the pinned version)
```rust
#[derive(Deserialize)]
struct ActiveBlock { total_tokens: u64, cost_usd: f64, started_at: String /* … */ }

#[derive(Serialize, Clone)]
struct GaugeState { used: u64, budget: u64, percent: f64,
                    burn_per_min: f64, eta_minutes: Option<f64>, cost_today: f64 }
```
Compute `GaugeState` in Rust so the **tray and the UI share one source of truth** (the tray is Rust-owned).

**Tray** (`TrayIconBuilder`)
- Three **prebuilt** icons bundled (green/amber/red); pick by `percent` band; set the tooltip each tick to `"58% of 5h block · ~1h12m left · $3.40 today"`.
- Left-click → toggle the main window (`show()`/`hide()`); right-click menu (Open/Refresh/Quit) via `MenuBuilder`.
- ponytail: 3 static icons, not a runtime-rendered gauge — the color band is the whole signal.

**Commands** (`#[tauri::command]`, called from SvelteKit via `invoke`)
- `get_settings()` / `set_settings(budget, intervals)`
- `refresh_now()`
- `ccusage_status()` → `{ found: bool, version: String }` for the Settings screen.

**Frontend ↔ Rust contract**
- Frontend listens (`@tauri-apps/api/event`): `ccm://active-block`, `ccm://usage`, `ccm://error`.
- Frontend calls (`@tauri-apps/api/core` `invoke`): the four commands above.

**Project layout**
```
ccm/
  src/              # SvelteKit frontend
  src-tauri/
    src/main.rs     # setup, polling tasks, tray, commands
    src/ccusage.rs  # Windows-safe spawn + serde parse
    src/gauge.rs    # burn-rate / ETA math (pure, unit-tested)
    tauri.conf.json
```
ponytail: put the burn-rate/ETA math in `gauge.rs` as pure functions with `#[cfg(test)]` asserts — the one piece worth a Rust test.

---

## Views (keep it to four)
1. **Overview** — the big live **5-hour-block gauge** (ring: % of your budget, burn rate, "ceiling at ~3:40pm" forecast), plus today's cost + tokens and this week's cost.
2. **Sessions** — recent sessions table/timeline (project, model, tokens, cost, duration).
3. **Breakdown** — by project and by model (bars); cache vs fresh token split.
4. **Settings** — ccusage path/version, your 5-hour token budget, poll intervals, manual refresh, empty-state help.

## System tray (the reason it's an app, not a CLI)
- **Color-coded tray icon** (green → amber → red by % of block budget) + **tooltip**: `"58% of 5h block · ~1h12m left · $3.40 today"`.
- **Left-click:** toggle the main window. **Right-click menu:** Open · Refresh · Quit.
- ponytail: color icon + tooltip, not a live-rendered numeric badge — Windows tray text badges are fiddly and the color is what you glance at anyway. This is the ambient "see your limit" value the CLIs can't give you.

---

## Explicit non-goals (keep it lean + low-maintenance)
- **No re-parsing `~/.claude` ourselves** — ccusage owns that, forever.
- No hosted backend, no accounts, no team/multi-user, no auth.
- No OTel in v1 (it's the staged real-time upgrade — see below).
- No own historical database — the local files + ccusage *are* the store; we just read. (Optional: cache the last JSON to disk for instant startup. ponytail: skip unless cold start feels slow.)
- No cross-platform polish in v1 (Windows-first).

## Setup / calibration (must do by hand)
- **Node + ccusage available.** Detect `ccusage` on PATH; else fall back to `npx ccusage@<pinned>`. On Windows the spawn is `npx.cmd` / `cmd /c npx ...` — handle it in the Rust `Command`.
- **Rust toolchain:** `rustup` + stable Rust, and on Windows the **MSVC C++ build tools** (Visual Studio "Desktop development with C++") + **WebView2** (preinstalled on Win11). Install the Tauri CLI (`cargo install tauri-cli` or `@tauri-apps/cli`); scaffold with `npm create tauri-app` (choose SvelteKit) or `tauri init` over the existing SvelteKit app.
- **Graceful states:** "ccusage not found" (link to install), "no Claude Code data yet" (empty state) — never crash on either.
- **Pin the ccusage version**, show it in Settings, re-test the JSON shape when you bump it.
- Set your **5-hour token budget** in Settings (no API for it).

## Product-upgrade path (staged, only if it grows beyond personal)
- **Official real-time via OTel:** set `CLAUDE_CODE_ENABLE_TELEMETRY=1` and run a tiny **OTLP receiver in Rust** to get exact live `claude_code.token.usage` / `claude_code.cost.usage` independent of ccusage. More setup; do it only if ccusage polling proves insufficient.
- **Packaging:** signed installer + auto-update.
- **If productizing:** confirm ccusage's license permits bundling (or shell out to the user's own install); your only real differentiator is **design/UX polish** over the CLIs. Treat revenue as upside — ship free first.

## Verification checklist
- [ ] ccusage detected; version shown in Settings.
- [ ] Overview "today" cost matches `ccusage daily` run in a terminal.
- [ ] Active-block gauge updates within one poll after a Claude Code turn.
- [ ] Burn-rate forecast is sane; gauge ceiling uses your set budget; UI labels it an estimate.
- [ ] Tray icon color tracks %, tooltip accurate, left-click toggles window, Quit works.
- [ ] Empty state shown when no `~/.claude` data; clear error (not a crash) if Node/ccusage missing.

## Build prompt (hand to Fable)
> Build a Tauri v2 + SvelteKit **Windows desktop app** called ccm that visualizes `ccusage` JSON output. Rust side: on two timers (5–10s for `ccusage blocks --active --json`, 30–60s for `ccusage daily --json` and `ccusage session --json`), spawn ccusage (detect on PATH, else `npx ccusage@<PINNED>`, Windows-safe spawn), parse the JSON, and emit Tauri events to the frontend. Do NOT parse `~/.claude` files directly — ccusage is the only data source. SvelteKit frontend: four views — Overview (live 5-hour-block ring gauge with burn rate + ETA-to-ceiling forecast, plus today/week cost), Sessions, Breakdown (by project/model, cache vs fresh), Settings (ccusage version, user-set 5-hour token budget, poll intervals). The block gauge ceiling comes from the user-set budget (no plan-limit API exists — label the gauge an estimate). System tray: color-coded icon (green/amber/red by % of budget) + tooltip ("X% of 5h block · ~Ym left · $Z today"), left-click toggles window, right-click menu Open/Refresh/Quit. Handle "ccusage not found" and "no data yet" gracefully. No backend, no accounts, no OTel. Structure the Rust in `src-tauri/` as `main.rs` (setup, tokio interval tasks, tray, commands), `ccusage.rs` (Windows-safe spawn + serde parse), and `gauge.rs` (pure burn-rate/ETA math with `#[cfg(test)]` asserts). Use `tauri-plugin-store` for settings, three prebuilt tray icons chosen by percent band, and emit `ccm://active-block` / `ccm://usage` / `ccm://error`. Mark anything needing the Rust toolchain / signing / a pinned ccusage version with `// SETUP REQUIRED`.

---

## Alternative delivery — browser + sidecar (if you prefer no Rust)
Pure-web option: a static **SvelteKit** dashboard + a ~40-line **Bun/Node sidecar** that runs `ccusage --json` on the same two intervals and serves it over `localhost` (JSON + SSE for live updates). Same views, same ccusage contract, no tray. Lazier toolchain, but you start a local process and lose the tray gauge (the best part). Use this only if the Rust toolchain isn't worth it to you.
