//! Windows-safe spawning of the ccusage CLI — the app's only data source.

use serde_json::Value;
use std::process::Stdio;
use tokio::process::Command;

// SETUP REQUIRED: pinned ccusage version, used when `ccusage` is not on PATH.
// JSON shapes in this app were verified against 20.0.14 — re-verify
// (claude blocks/daily/session --json) before bumping.
pub const PINNED_VERSION: &str = "20.0.14";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Runner {
    /// `ccusage` found on PATH
    PathBin,
    /// fall back to `npx -y ccusage@PINNED_VERSION`
    Npx,
}

impl Runner {
    pub fn describe(self) -> String {
        match self {
            Runner::PathBin => "PATH".into(),
            Runner::Npx => format!("npx (ccusage@{PINNED_VERSION})"),
        }
    }
}

fn base(runner: Runner) -> Command {
    // Windows: npm installs ccusage as a .cmd shim and npx itself is npx.cmd —
    // neither spawns directly, so go through cmd.exe. CREATE_NO_WINDOW stops a
    // console window flashing on every poll.
    #[cfg(windows)]
    {
        let mut c = Command::new("cmd");
        c.arg("/C");
        match runner {
            Runner::PathBin => {
                c.arg("ccusage");
            }
            Runner::Npx => {
                c.args(["npx", "-y", &format!("ccusage@{PINNED_VERSION}")]);
            }
        }
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        c
    }
    #[cfg(not(windows))]
    {
        match runner {
            Runner::PathBin => Command::new("ccusage"),
            Runner::Npx => {
                let mut c = Command::new("npx");
                c.args(["-y", &format!("ccusage@{PINNED_VERSION}")]);
                c
            }
        }
    }
}

async fn run(runner: Runner, args: &[&str]) -> Result<String, String> {
    let out = base(runner)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("failed to spawn ccusage: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "ccusage {} failed ({}): {}",
            args.join(" "),
            out.status,
            err.trim().lines().next().unwrap_or("")
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub async fn run_json(runner: Runner, args: &[&str]) -> Result<Value, String> {
    let raw = run(runner, args).await?;
    // npm/npx may prefix notices to stdout — start at the first '{'
    let start = raw.find('{').ok_or("no JSON in ccusage output")?;
    serde_json::from_str(&raw[start..]).map_err(|e| format!("bad JSON from ccusage: {e}"))
}

/// Try `ccusage` on PATH, then the pinned npx fallback. Returns the working
/// runner and its version string. Called once and cached.
pub async fn detect() -> Result<(Runner, String), String> {
    for runner in [Runner::PathBin, Runner::Npx] {
        if let Ok(v) = run(runner, &["--version"]).await {
            let v = v.trim();
            let version = v.strip_prefix("ccusage ").unwrap_or(v).to_string();
            return Ok((runner, version));
        }
    }
    Err(format!(
        "ccusage not found. Install Node.js and run `npm i -g ccusage`, or make sure `npx` works \
         (ccm falls back to `npx ccusage@{PINNED_VERSION}`)."
    ))
}
