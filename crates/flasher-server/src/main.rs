//! Thin entry point: parse environment config, init tracing, connect the
//! store, build the auth driver, bind, serve.
//!
//! Auth modes: `FLASHER_USER` set → dev bypass (every request acts as
//! that user, no sessions). Unset → passkey auth: `/api/*` (except
//! `/api/health` and `/api/auth/*`) requires a valid session cookie;
//! registration of the first passkey is open (optionally gated by
//! `FLASHER_BOOTSTRAP_TOKEN`).

use std::{net::SocketAddr, path::PathBuf};

use flasher_auth::Auth;
use flasher_core::SrsConfig;
use flasher_server::{AppState, DEFAULT_PAGE_SIZE};
use flasher_store::Store;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

const DEFAULT_PORT: u16 = 3000;
const DEFAULT_DIST: &str = "frontends/leptos/dist";
const DEFAULT_DB: &str = "flasher.db";
const DEFAULT_RP_ID: &str = "localhost";
const DEFAULT_ORIGIN: &str = "http://localhost:3000";
const RP_NAME: &str = "flasher";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let port = std::env::var("FLASHER_PORT").map_or(DEFAULT_PORT, |raw| {
        raw.parse().unwrap_or_else(|_| {
            tracing::warn!(%raw, default = DEFAULT_PORT, "invalid FLASHER_PORT, using default");
            DEFAULT_PORT
        })
    });
    let dist_dir =
        std::env::var("FLASHER_DIST").map_or_else(|_| PathBuf::from(DEFAULT_DIST), PathBuf::from);
    let db_path =
        std::env::var("FLASHER_DB").map_or_else(|_| PathBuf::from(DEFAULT_DB), PathBuf::from);
    let srs = srs_config_from(|name| std::env::var(name).ok());
    let page_size = std::env::var("FLASHER_PAGE_SIZE").map_or(DEFAULT_PAGE_SIZE, |raw| {
        parse_positive_u32(&raw).unwrap_or_else(|| {
            tracing::warn!(%raw, default = DEFAULT_PAGE_SIZE, "invalid FLASHER_PAGE_SIZE, using default");
            DEFAULT_PAGE_SIZE
        })
    });

    let store = Store::connect(&db_path).await?;
    let swept = store.delete_expired_sessions(now_millis()).await?;
    if swept > 0 {
        tracing::info!(swept, "deleted expired sessions at startup");
    }

    let rp_id = std::env::var("FLASHER_RP_ID").unwrap_or_else(|_| DEFAULT_RP_ID.to_owned());
    let origin = std::env::var("FLASHER_ORIGIN").unwrap_or_else(|_| DEFAULT_ORIGIN.to_owned());
    let auth = Auth::new(&rp_id, &origin, RP_NAME)?;

    let state = if let Ok(username) = std::env::var("FLASHER_USER") {
        let user = store.upsert_user(&username).await?;
        tracing::info!(user = %user.username, "auth: dev bypass (FLASHER_USER) — no session required");
        AppState::dev_bypass(store, auth, user.id)
    } else {
        let bootstrap_token = std::env::var("FLASHER_BOOTSTRAP_TOKEN").ok().and_then(|token| {
            if is_placeholder_bootstrap_token(&token) {
                tracing::warn!(
                    "FLASHER_BOOTSTRAP_TOKEN is the publicly known placeholder from \
                     deploy/flasher.service — treating it as UNSET. Set a real random token."
                );
                None
            } else {
                Some(token)
            }
        });
        if bootstrap_token.is_none() {
            tracing::warn!(
                "FLASHER_BOOTSTRAP_TOKEN is unset: while the system has ZERO passkeys, \
                 ANYONE who can reach this server can register the first passkey \
                 (open bootstrap). Set FLASHER_BOOTSTRAP_TOKEN to gate it."
            );
        }
        tracing::info!(%rp_id, %origin, "auth: passkey mode — session required for /api/*");
        AppState::new(store, auth).with_bootstrap_token(bootstrap_token)
    };
    let state = state.with_srs_config(srs).with_page_size(page_size);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, dist = %dist_dir.display(), db = %db_path.display(), "flasher listening");
    flasher_server::serve(listener, dist_dir, state).await?;
    Ok(())
}

/// SRS scheduling parameters from a config-value lookup (the environment
/// in `main`, a map in tests): `FLASHER_OK_MULTIPLIER`,
/// `FLASHER_FAILED_MULTIPLIER` and `FLASHER_NEW_CARD_WAITING_MS`, falling
/// back to the compiled-in defaults (with a warning) on invalid values.
fn srs_config_from(get: impl Fn(&str) -> Option<String>) -> SrsConfig {
    let mut config = SrsConfig::default();
    if let Some(value) = lookup_env::<f64>(&get, "FLASHER_OK_MULTIPLIER") {
        config.ok_multiplier = value;
    }
    if let Some(value) = lookup_env::<f64>(&get, "FLASHER_FAILED_MULTIPLIER") {
        config.failed_multiplier = value;
    }
    if let Some(value) = lookup_env::<i64>(&get, "FLASHER_NEW_CARD_WAITING_MS") {
        config.new_card_waiting_ms = value;
    }
    config
}

/// Looks up and parses one config value: unset yields `None` (the caller
/// keeps the default), an invalid value warns and yields `None`.
fn lookup_env<T: std::str::FromStr>(
    get: &impl Fn(&str) -> Option<String>,
    name: &str,
) -> Option<T> {
    let raw = get(name)?;
    if let Ok(value) = raw.parse::<T>() {
        Some(value)
    } else {
        tracing::warn!(%raw, %name, "invalid value, keeping the default");
        None
    }
}

/// Parses a strictly positive `u32` config value (`FLASHER_PAGE_SIZE`);
/// zero, negative and non-numeric values are rejected.
fn parse_positive_u32(raw: &str) -> Option<u32> {
    match raw.parse::<u32>() {
        Ok(value) if value > 0 => Some(value),
        _ => None,
    }
}

/// The placeholder shipped in deploy/flasher.service is public knowledge:
/// honoring it would protect nothing while looking protected, so `main`
/// treats it as unset (with a loud warning).
fn is_placeholder_bootstrap_token(token: &str) -> bool {
    token == "CHANGE_ME_LONG_RANDOM"
}

/// Current time as unix epoch millis, falling back to 0 if the system
/// clock is before the epoch.
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    //! Unit tests for the pure config helpers (`main` itself is process
    //! wiring, covered by the e2e boot).

    use super::*;

    #[test]
    fn parse_positive_u32_accepts_only_positive_values() {
        assert_eq!(parse_positive_u32("1"), Some(1));
        assert_eq!(parse_positive_u32("42"), Some(42));
        assert_eq!(parse_positive_u32("0"), None);
        assert_eq!(parse_positive_u32("-1"), None);
        assert_eq!(parse_positive_u32("abc"), None);
        assert_eq!(parse_positive_u32(""), None);
    }

    #[test]
    fn placeholder_bootstrap_token_is_detected() {
        assert!(is_placeholder_bootstrap_token("CHANGE_ME_LONG_RANDOM"));
        assert!(!is_placeholder_bootstrap_token("change_me_long_random"));
        assert!(!is_placeholder_bootstrap_token("a-real-random-token"));
        assert!(!is_placeholder_bootstrap_token(""));
    }

    /// Exact float equality is intentional here: the config either
    /// applies the parsed value verbatim or keeps the compiled-in
    /// default constant.
    #[allow(clippy::float_cmp)]
    fn assert_config_eq(actual: SrsConfig, expected: SrsConfig) {
        assert_eq!(actual.ok_multiplier, expected.ok_multiplier);
        assert_eq!(actual.failed_multiplier, expected.failed_multiplier);
        assert_eq!(actual.new_card_waiting_ms, expected.new_card_waiting_ms);
    }

    #[test]
    fn srs_config_reads_valid_values_and_keeps_defaults_otherwise() {
        let default = SrsConfig::default();
        let config = srs_config_from(|name| match name {
            "FLASHER_OK_MULTIPLIER" => Some("2.5".to_owned()),
            "FLASHER_FAILED_MULTIPLIER" => Some("not-a-number".to_owned()),
            "FLASHER_NEW_CARD_WAITING_MS" => Some("60000".to_owned()),
            _ => None,
        });
        // Valid values are applied; invalid ones keep the default.
        assert_config_eq(
            config,
            SrsConfig {
                ok_multiplier: 2.5,
                failed_multiplier: default.failed_multiplier,
                new_card_waiting_ms: 60_000,
            },
        );
    }

    #[test]
    fn srs_config_is_the_default_when_everything_is_unset() {
        assert_config_eq(srs_config_from(|_| None), SrsConfig::default());
    }

    #[test]
    fn now_millis_is_unix_epoch_millis() {
        // Any clock past 2001-09-09 is past a trillion millis.
        assert!(now_millis() > 1_000_000_000_000);
    }
}
