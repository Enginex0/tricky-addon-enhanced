use anyhow::{Context, Result};
use tracing::{info, warn};

use crate::platform::network;

const BULLETIN_URL: &str = "https://source.android.com/docs/security/bulletin/pixel";

const FALLBACK_PATCHES: &[&str] = &[
    "2026-03-01",
    "2026-02-01",
    "2026-01-01",
    "2025-12-01",
    "2025-11-01",
    "2025-10-01",
];

pub fn fetch_latest_patch() -> Result<String> {
    match fetch_from_bulletin() {
        Ok(date) => {
            info!("fetched latest patch date from bulletin: {date}");
            Ok(date)
        }
        Err(e) => {
            warn!("bulletin scrape failed: {e}, using fallback table");
            fallback_patch_date()
        }
    }
}

fn fetch_from_bulletin() -> Result<String> {
    let html =
        network::download_text(BULLETIN_URL).context("failed to download security bulletin")?;
    parse_patch_date(&html)
}

fn parse_patch_date(html: &str) -> Result<String> {
    for line in html.lines() {
        if let Some(date) = extract_date_from_td(line) {
            return Ok(date);
        }
    }
    for line in html.lines() {
        if let Some(date) = find_date_pattern(line) {
            return Ok(date);
        }
    }
    anyhow::bail!("no patch date found in bulletin HTML")
}

fn extract_date_from_td(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with("<td>") {
        return None;
    }
    let content = trimmed.strip_prefix("<td>")?.strip_suffix("</td>")?.trim();
    if is_valid_patch_date(content) {
        Some(content.to_string())
    } else {
        None
    }
}

fn find_date_pattern(line: &str) -> Option<String> {
    let chars: Vec<char> = line.chars().collect();
    if chars.len() < 10 {
        return None;
    }
    for i in 0..=chars.len() - 10 {
        let candidate: String = chars[i..i + 10].iter().collect();
        if is_valid_patch_date(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_valid_patch_date(s: &str) -> bool {
    if s.len() != 10 {
        return false;
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    parts[0].len() == 4
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && parts[1].chars().all(|c| c.is_ascii_digit())
        && parts[2].chars().all(|c| c.is_ascii_digit())
        && parts[0].starts_with("20")
}

fn fallback_patch_date() -> Result<String> {
    if let Some(latest) = FALLBACK_PATCHES.first() {
        if is_stale(latest) {
            let synthetic = derive_current_month_date();
            warn!("fallback table is stale, using derived date: {synthetic}");
            return Ok(synthetic);
        }
        warn!("using hardcoded fallback patch date: {latest}");
        return Ok(latest.to_string());
    }
    anyhow::bail!("no fallback patch dates available")
}

fn is_stale(date: &str) -> bool {
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() != 3 {
        return true;
    }
    let year: i32 = parts[0].parse().unwrap_or(0);
    let month: i32 = parts[1].parse().unwrap_or(0);
    let date_months = year * 12 + month;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let approx_year = 1970 + (now / 31_557_600) as i32;
    let approx_month = ((now % 31_557_600) / 2_629_800) as i32 + 1;
    let now_months = approx_year * 12 + approx_month;

    now_months - date_months > 3
}

fn derive_current_month_date() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let year = 1970 + now / 31_557_600;
    let month = (now % 31_557_600) / 2_629_800 + 1;
    format!("{}-{:02}-01", year, month.min(12))
}
