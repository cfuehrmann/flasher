//! Smoke test: the app boots in a real browser and completes the
//! same-origin `/api/health` round-trip — exercised through the DOM,
//! exactly what a user sees. This is the project's primary test
//! surface; the HTTP API itself is internal and never asserted here.

use std::time::Duration;

use flasher_e2e::{Error, Result, TestHarness};

#[tokio::test]
#[ignore = "browser"]
async fn home_boots_and_shows_server_health() -> Result<()> {
    let h = TestHarness::start().await?;
    h.goto("/").await?;

    // The wasm bundle booted, fetched /api/health from the same origin,
    // and rendered the server's status + version into the page.
    h.wait_for_text("p.health", "status: ok", Duration::from_secs(15))
        .await?;
    h.wait_for_text("p.health", "version:", Duration::from_secs(5))
        .await?;

    let title = h.title().await?;
    if title.as_deref() != Some("Flasher") {
        return Err(Error::message(format!(
            "expected page title \"Flasher\", got {title:?}"
        )));
    }
    let body = h.page_text().await?;
    if !body.contains("Flasher") {
        return Err(Error::message(format!(
            "expected rendered page to contain \"Flasher\", got: {body:?}"
        )));
    }

    h.screenshot("01_smoke/home").await?;
    Ok(())
}
