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
    h.wait_for_text("header.top h1", "Quiz", Duration::from_secs(5))
        .await?;

    let title = h.title().await?;
    if title.as_deref() != Some("Flasher") {
        return Err(Error::message(format!(
            "expected page title \"Flasher\", got {title:?}"
        )));
    }
    // The logo remains the accessible Flasher brand while the adjacent h1
    // carries the current page title.
    let logo_alt: Option<String> = h
        .eval("document.querySelector('header.top img.brand-logo')?.getAttribute('alt')")
        .await?;
    if logo_alt.as_deref() != Some("Flasher") {
        return Err(Error::message(format!(
            "expected header logo alt=\"Flasher\", got {logo_alt:?}"
        )));
    }

    h.screenshot("01_smoke/home").await?;
    Ok(())
}
