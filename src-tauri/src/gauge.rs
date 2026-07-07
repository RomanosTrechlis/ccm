//! Pure burn-rate / ETA math. No I/O — the one piece worth unit tests.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The subset of a ccusage block we do math on. The raw JSON is passed to the
/// frontend untouched; this struct only feeds the gauge + tray.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(default, rename = "costUSD")]
    pub cost_usd: f64,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub burn_rate: Option<BurnRate>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BurnRate {
    #[serde(default)]
    pub tokens_per_minute: f64,
    #[serde(default)]
    pub cost_per_hour: f64,
}

/// Computed in Rust so the tray and the UI share one source of truth.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GaugeState {
    pub active: bool,
    pub used: u64,
    pub budget: u64,
    pub percent: f64,
    pub burn_per_min: f64,
    pub cost_per_hour: f64,
    /// minutes until `used` hits `budget` at the current burn rate
    pub eta_minutes: Option<f64>,
    pub block_cost: f64,
    pub block_end: Option<String>,
    pub block_remaining_minutes: Option<f64>,
    pub cost_today: f64,
}

pub fn compute(block: Option<&Block>, budget: u64, cost_today: f64, now: DateTime<Utc>) -> GaugeState {
    let base = GaugeState { budget, cost_today, ..Default::default() };
    let b = match block {
        Some(b) if b.is_active => b,
        _ => return base,
    };

    let used = b.total_tokens;
    let percent = if budget > 0 { used as f64 / budget as f64 } else { 0.0 };

    // prefer ccusage's burn rate; derive from elapsed time if it's missing
    let elapsed_min = b
        .start_time
        .as_deref()
        .and_then(parse_iso)
        .map(|st| (now - st).num_seconds() as f64 / 60.0)
        .filter(|m| *m > 0.0);
    let burn_per_min = b
        .burn_rate
        .as_ref()
        .map(|r| r.tokens_per_minute)
        .filter(|t| *t > 0.0)
        .or_else(|| elapsed_min.map(|m| used as f64 / m))
        .unwrap_or(0.0);

    let eta_minutes =
        (burn_per_min > 0.0 && used < budget).then(|| (budget - used) as f64 / burn_per_min);
    let block_remaining_minutes = b
        .end_time
        .as_deref()
        .and_then(parse_iso)
        .map(|et| ((et - now).num_seconds() as f64 / 60.0).max(0.0));

    GaugeState {
        active: true,
        used,
        percent,
        burn_per_min,
        cost_per_hour: b.burn_rate.as_ref().map(|r| r.cost_per_hour).unwrap_or(0.0),
        eta_minutes,
        block_cost: b.cost_usd,
        block_end: b.end_time.clone(),
        block_remaining_minutes,
        ..base
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Band {
    Grey,
    Green,
    Amber,
    Red,
}

pub fn color_band(g: &GaugeState, has_error: bool) -> Band {
    if has_error || !g.active {
        Band::Grey
    } else if g.percent >= 0.85 {
        Band::Red
    } else if g.percent >= 0.60 {
        Band::Amber
    } else {
        Band::Green
    }
}

pub fn tooltip(g: &GaugeState, error: Option<&str>) -> String {
    if let Some(e) = error {
        // chars, not bytes — byte slicing can panic mid-UTF-8
        let first: String = e.lines().next().unwrap_or(e).chars().take(100).collect();
        return format!("ccm — {first}");
    }
    if !g.active {
        return format!("ccm — no active block · ${:.2} today", g.cost_today);
    }
    format!(
        "ccm — {:.0}% of 5h block · ~{} left · ${:.2} today",
        g.percent * 100.0,
        g.block_remaining_minutes.map(fmt_mins).unwrap_or_else(|| "?".into()),
        g.cost_today
    )
}

fn fmt_mins(m: f64) -> String {
    let m = m.max(0.0).round() as u64;
    if m >= 60 {
        format!("{}h {:02}m", m / 60, m % 60)
    } else {
        format!("{m}m")
    }
}

fn parse_iso(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_block() -> Block {
        // shape verified against ccusage 20.0.14 `claude blocks --active --json`
        serde_json::from_str(
            r#"{
                "isActive": true,
                "totalTokens": 5000000,
                "costUSD": 15.5,
                "startTime": "2026-07-06T17:00:00.000Z",
                "endTime": "2026-07-06T22:00:00.000Z",
                "burnRate": { "tokensPerMinute": 50000, "costPerHour": 20.0 }
            }"#,
        )
        .unwrap()
    }

    fn t(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn gauge_math() {
        let g = compute(Some(&sample_block()), 10_000_000, 12.5, t("2026-07-06T18:00:00Z"));
        assert!(g.active);
        assert!((g.percent - 0.5).abs() < 1e-9);
        // (10M - 5M) / 50k per min = 100 min to ceiling
        assert!((g.eta_minutes.unwrap() - 100.0).abs() < 1e-9);
        // block ends 22:00, now 18:00 -> 240 min
        assert!((g.block_remaining_minutes.unwrap() - 240.0).abs() < 1e-9);
        assert_eq!(g.block_cost, 15.5);
        assert_eq!(g.cost_today, 12.5);
    }

    #[test]
    fn no_block_and_over_budget() {
        let g = compute(None, 10_000_000, 3.0, t("2026-07-06T18:00:00Z"));
        assert!(!g.active);
        assert_eq!(color_band(&g, false), Band::Grey);
        assert!(tooltip(&g, None).contains("no active block"));

        let over = compute(Some(&sample_block()), 4_000_000, 0.0, t("2026-07-06T18:00:00Z"));
        assert!(over.percent > 1.0);
        assert_eq!(over.eta_minutes, None); // already past the ceiling
        assert_eq!(color_band(&over, false), Band::Red);
    }

    #[test]
    fn burn_rate_derived_when_missing() {
        let mut b = sample_block();
        b.burn_rate = None;
        // 5M tokens over 60 elapsed minutes
        let g = compute(Some(&b), 10_000_000, 0.0, t("2026-07-06T18:00:00Z"));
        assert!((g.burn_per_min - 5_000_000.0 / 60.0).abs() < 1.0);
    }

    #[test]
    fn bands_and_tooltip() {
        let mk = |percent| GaugeState { active: true, percent, ..Default::default() };
        assert_eq!(color_band(&mk(0.2), false), Band::Green);
        assert_eq!(color_band(&mk(0.7), false), Band::Amber);
        assert_eq!(color_band(&mk(0.9), false), Band::Red);
        assert_eq!(color_band(&mk(0.2), true), Band::Grey);

        let g = GaugeState {
            active: true,
            percent: 0.58,
            block_remaining_minutes: Some(72.0),
            cost_today: 3.4,
            ..Default::default()
        };
        assert_eq!(tooltip(&g, None), "ccm — 58% of 5h block · ~1h 12m left · $3.40 today");
    }
}
