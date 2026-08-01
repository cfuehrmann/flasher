//! Markdown + `KaTeX` e2e tests (Phase 4A): card content is rendered as
//! GFM Markdown (tables, bold, lists) and math is typeset by `KaTeX` —
//! driven through the browser, asserting on the real DOM. A second test
//! seeds hostile markup and verifies the pulldown-cmark → ammonia
//! pipeline keeps it inert.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use flasher_e2e::{E2E_USER, Error, Result, TestHarness};
use flasher_store::{CardState, NewCard, Store};

/// Timeout for every DOM wait; generous because the wasm bundle has to
/// download and boot first (same reasoning as the harness default).
const TIMEOUT: Duration = Duration::from_secs(15);

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(0))
}

// The error is only formatted, but `map_err` needs an owned receiver.
#[allow(clippy::needless_pass_by_value)]
fn store_err(err: flasher_store::Error) -> Error {
    Error::message(format!("store error: {err}"))
}

/// Seeds one due, enabled card and returns nothing; the quiz picks it up
/// immediately.
async fn seed_due_card(h: &TestHarness, id: &str, prompt: &str, solution: &str) -> Result<()> {
    let store: Store = h.seed_store().await.map_err(store_err)?;
    let user = store
        .get_user_by_name(E2E_USER)
        .await
        .map_err(store_err)?
        .ok_or_else(|| Error::message(format!("user {E2E_USER} not found")))?;
    let now = now_ms();
    store
        .insert_card(&NewCard {
            user_id: user.id,
            id: id.to_owned(),
            prompt: prompt.to_owned(),
            solution: solution.to_owned(),
            state: CardState::New,
            change_time: now - 60_000,
            next_time: now - 1_000,
            labels: vec!["Enabled".to_owned()],
        })
        .await
        .map_err(store_err)
}

/// Number of elements matching `sel` in the live DOM.
async fn count(h: &TestHarness, sel: &str) -> Result<u32> {
    h.eval::<u32>(&format!("document.querySelectorAll({sel:?}).length"))
        .await
}

/// Polls a JS boolean expression until it holds (`KaTeX` typesets
/// asynchronously after the DOM update) or the deadline elapses.
async fn wait_for_js(h: &TestHarness, expr: &str, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(true) = h.eval::<bool>(expr).await {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(Error::message(format!(
                "timed out waiting for JS condition: {expr}"
            )));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// A due card whose prompt carries a GFM table, bold and a list renders
/// as real `<table>`/`<strong>`/`<li>` elements; revealing the solution
/// typesets its inline and display math with `KaTeX`.
#[tokio::test]
#[ignore = "browser"]
async fn quiz_renders_markdown_and_katex() -> Result<()> {
    let h = TestHarness::start().await?;
    let prompt = "**ICMP types** worth knowing:\n\n\
                  | Type | Code | Description |\n\
                  |------|------|-------------|\n\
                  | 0 | 0 | Echo Reply |\n\
                  | 8 | 0 | Echo Request |\n\n\
                  - ping uses Echo Request\n\
                  - traceroute relies on TTL exceeded";
    let solution = "The square is $x^2$, the fraction is below.\n\n$$\n\\frac{a}{b}\n$$";
    seed_due_card(&h, "card-md", prompt, solution).await?;

    h.goto("/").await?;
    h.wait_for_text("#quiz-prompt", "ICMP types", TIMEOUT)
        .await?;

    if count(&h, "#quiz-prompt table").await? != 1 {
        return Err(Error::message("prompt should contain one <table>"));
    }
    if count(&h, "#quiz-prompt strong").await? < 1 {
        return Err(Error::message(
            "prompt should contain <strong>bold</strong>",
        ));
    }
    if count(&h, "#quiz-prompt li").await? < 2 {
        return Err(Error::message("prompt should contain the two <li> items"));
    }
    h.screenshot("05_markdown/prompt").await?;

    h.click("#show-solution").await?;
    h.wait_for_text("#quiz-solution", "square", TIMEOUT).await?;
    // KaTeX runs after the DOM update and possibly after a short wait
    // for the deferred scripts: poll for its output elements.
    wait_for_js(
        &h,
        "document.querySelectorAll('#quiz-solution .katex').length > 0",
        TIMEOUT,
    )
    .await?;
    wait_for_js(
        &h,
        "document.querySelectorAll('#quiz-solution .katex-display').length > 0",
        TIMEOUT,
    )
    .await?;
    h.screenshot("05_markdown/solution").await?;
    Ok(())
}

/// A bare URL in card content becomes a real link in the quiz (the old
/// app's remark-gfm autolinked bare URLs; the new pipeline wraps them in
/// `CommonMark` autolinks pre-parse). The trailing sentence period must
/// not end up in the `href`.
#[tokio::test]
#[ignore = "browser"]
async fn quiz_autolinks_bare_url() -> Result<()> {
    let h = TestHarness::start().await?;
    let prompt = "Reference: https://example.com/spec for details.";
    seed_due_card(&h, "card-url", prompt, "the solution").await?;

    h.goto("/").await?;
    h.wait_for_text("#quiz-prompt", "Reference", TIMEOUT)
        .await?;

    wait_for_js(
        &h,
        "document.querySelector('#quiz-prompt a[href=\"https://example.com/spec\"]')?.textContent === 'https://example.com/spec'",
        TIMEOUT,
    )
    .await?;
    h.screenshot("05_markdown/autolink").await?;
    Ok(())
}

/// A math-free card never injects the `KaTeX` assets: `needs_katex`
/// keeps `#katex-js` and `#katex-css` out of `<head>` entirely (the
/// positive case is covered by `quiz_renders_markdown_and_katex`).
#[tokio::test]
#[ignore = "browser"]
async fn quiz_without_math_loads_no_katex() -> Result<()> {
    let h = TestHarness::start().await?;
    seed_due_card(
        &h,
        "card-plain",
        "A plain prompt without math.",
        "A plain solution.",
    )
    .await?;

    h.goto("/").await?;
    h.wait_for_text("#quiz-prompt", "plain prompt", TIMEOUT)
        .await?;
    h.click("#show-solution").await?;
    h.wait_for_text("#quiz-solution", "plain solution", TIMEOUT)
        .await?;

    // Give any (wrong) lazy injection a beat to happen.
    tokio::time::sleep(Duration::from_millis(500)).await;
    if h.eval::<bool>("!!document.head.querySelector('#katex-js')")
        .await?
    {
        return Err(Error::message("math-free card must not inject #katex-js"));
    }
    if h.eval::<bool>("!!document.head.querySelector('#katex-css')")
        .await?
    {
        return Err(Error::message("math-free card must not inject #katex-css"));
    }
    Ok(())
}

/// Hostile markup in card content is neutralized by the
/// pulldown-cmark → ammonia pipeline before it reaches `inner_html`:
/// `<script>` elements never exist in the DOM, event-handler attributes
/// are stripped, and neither payload fires (the `window.__pwned`
/// tripwire stays unset).
#[tokio::test]
#[ignore = "browser"]
async fn quiz_sanitizes_hostile_markup() -> Result<()> {
    let h = TestHarness::start().await?;
    let prompt = "Safety check <script>window.__pwned=1</script> middle \
                  <img src=\"x\" onerror=\"window.__pwned=1\"> end";
    seed_due_card(&h, "card-xss", prompt, "harmless").await?;

    h.goto("/").await?;
    h.wait_for_text("#quiz-prompt", "Safety check", TIMEOUT)
        .await?;

    if count(&h, "#quiz-prompt script").await? != 0 {
        return Err(Error::message("<script> survived sanitization"));
    }
    if count(&h, "[onerror]").await? != 0 {
        return Err(Error::message("an onerror handler survived sanitization"));
    }
    // Give any stray payload a beat to fire, then check the tripwire.
    tokio::time::sleep(Duration::from_millis(250)).await;
    let pwned: bool = h
        .eval::<bool>("typeof window.__pwned !== 'undefined'")
        .await?;
    if pwned {
        return Err(Error::message(
            "injected script executed (window.__pwned set)",
        ));
    }
    h.screenshot("05_markdown/sanitized").await?;
    Ok(())
}
