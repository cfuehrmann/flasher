//! Browser regressions found through Chromium `DevTools` inspection.

use std::sync::Arc;
use std::time::Duration;

use chromiumoxide::cdp::js_protocol::runtime::{EventConsoleApiCalled, EventExceptionThrown};
use flasher_e2e::{Error, Result, TestHarness};
use futures::{Stream, StreamExt};

const TIMEOUT: Duration = Duration::from_secs(15);

async fn drain_console(
    console: &mut (impl Stream<Item = Arc<EventConsoleApiCalled>> + Unpin),
    exceptions: &mut (impl Stream<Item = Arc<EventExceptionThrown>> + Unpin),
) -> Vec<String> {
    let mut lines = Vec::new();
    loop {
        let mut received = false;
        if let Ok(Some(event)) =
            tokio::time::timeout(Duration::from_millis(100), console.next()).await
        {
            let args = event
                .args
                .iter()
                .filter_map(|arg| {
                    arg.value
                        .as_ref()
                        .map(ToString::to_string)
                        .or_else(|| arg.description.clone())
                })
                .collect::<Vec<_>>()
                .join(" | ");
            lines.push(format!("console {:?}: {args}", event.r#type));
            received = true;
        }
        if let Ok(Some(event)) =
            tokio::time::timeout(Duration::from_millis(100), exceptions.next()).await
        {
            let details = &event.exception_details;
            let description = details
                .exception
                .as_ref()
                .and_then(|exception| exception.description.as_deref())
                .unwrap_or("");
            lines.push(format!("exception {}: {description}", details.text));
            received = true;
        }
        if !received {
            break;
        }
    }
    lines
}

#[tokio::test]
#[ignore = "browser"]
async fn leaving_groom_during_search_debounce_is_console_clean() -> Result<()> {
    let h = TestHarness::start().await?;
    let mut console = h
        .page
        .event_listener::<EventConsoleApiCalled>()
        .await
        .map_err(Error::Cdp)?;
    let mut exceptions = h
        .page
        .event_listener::<EventExceptionThrown>()
        .await
        .map_err(Error::Cdp)?;

    h.goto("/").await?;
    h.click("#tab-groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;
    h.wait_for_selector("#groom-empty", TIMEOUT).await?;

    // Leave Groom before its 300 ms search debounce can fire. Before the
    // fix, the callback accessed a disposed RwSignal and Chromium reported
    // both a panic and a matching uncaught exception.
    h.type_into("#groom-search", "probe").await?;
    h.click("#groom-clear").await?;
    h.click("#tab-labels").await?;
    h.wait_for_selector("#labels-empty", TIMEOUT).await?;

    let messages = drain_console(&mut console, &mut exceptions).await;
    if messages
        .iter()
        .any(|message| message.contains("already been disposed"))
    {
        return Err(Error::message(format!(
            "Groom navigation emitted a disposed-reactive-value console error: {messages:?}"
        )));
    }
    Ok(())
}

#[tokio::test]
#[ignore = "browser"]
async fn leaving_groom_during_resize_debounce_is_console_clean() -> Result<()> {
    let h = TestHarness::start().await?;
    let mut console = h
        .page
        .event_listener::<EventConsoleApiCalled>()
        .await
        .map_err(Error::Cdp)?;
    let mut exceptions = h
        .page
        .event_listener::<EventExceptionThrown>()
        .await
        .map_err(Error::Cdp)?;

    h.goto("/").await?;
    h.click("#tab-groom").await?;
    h.wait_for_selector("#groom-search", TIMEOUT).await?;

    // A window drag dispatches a burst of resize events. Leave Groom while
    // those callbacks are still debounced. Before the fix, their generation
    // signal was disposed with the component and the timer then panicked.
    h.eval::<bool>(
        "(() => { for (let i = 0; i < 12; i++) window.dispatchEvent(new Event('resize')); return true; })()",
    )
    .await?;
    h.click("#tab-labels").await?;
    h.wait_for_selector("#labels-empty", TIMEOUT).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let messages = drain_console(&mut console, &mut exceptions).await;
    if messages
        .iter()
        .any(|message| message.contains("already been disposed"))
    {
        return Err(Error::message(format!(
            "Groom navigation emitted a disposed-reactive-value console error after resize: {messages:?}"
        )));
    }
    Ok(())
}

#[tokio::test]
#[ignore = "browser"]
async fn editor_labels_are_associated_with_controls() -> Result<()> {
    let h = TestHarness::start().await?;
    h.goto("/add").await?;
    h.wait_for_selector("#new-prompt", TIMEOUT).await?;

    let unassociated: Vec<String> = h
        .eval(
            r"Array.from(document.querySelectorAll('label'))
                .filter(label => !label.control)
                .map(label => (label.textContent || '').trim())",
        )
        .await?;
    if !unassociated.is_empty() {
        return Err(Error::message(format!(
            "editor contains labels without associated controls: {unassociated:?}"
        )));
    }
    Ok(())
}

#[tokio::test]
#[ignore = "browser"]
async fn wasm_preload_does_not_emit_unsupported_sri_warning() -> Result<()> {
    let h = TestHarness::start().await?;
    let mut console = h
        .page
        .event_listener::<EventConsoleApiCalled>()
        .await
        .map_err(Error::Cdp)?;
    let mut exceptions = h
        .page
        .event_listener::<EventExceptionThrown>()
        .await
        .map_err(Error::Cdp)?;

    h.goto("/quiz").await?;
    h.wait_for_selector("#tab-quiz", TIMEOUT).await?;

    let unsupported_preloads: usize = h
        .eval("document.querySelectorAll('link[rel=preload][as=fetch][integrity]').length")
        .await?;
    if unsupported_preloads != 0 {
        return Err(Error::message(
            "the WASM fetch preload must not carry an unsupported integrity attribute",
        ));
    }

    let messages = drain_console(&mut console, &mut exceptions).await;
    if messages
        .iter()
        .any(|message| message.contains("integrity") && message.contains("preload"))
    {
        return Err(Error::message(format!(
            "the WASM preload emitted an unsupported-SRI console warning: {messages:?}"
        )));
    }
    Ok(())
}

#[tokio::test]
#[ignore = "browser"]
async fn narrowing_viewport_moves_focus_before_hiding_navigation() -> Result<()> {
    let h = TestHarness::start().await?;
    let mut console = h
        .page
        .event_listener::<EventConsoleApiCalled>()
        .await
        .map_err(Error::Cdp)?;
    let mut exceptions = h
        .page
        .event_listener::<EventExceptionThrown>()
        .await
        .map_err(Error::Cdp)?;

    h.goto("/quiz").await?;
    h.wait_for_selector("#tab-quiz", TIMEOUT).await?;
    h.eval::<bool>("document.querySelector('#tab-quiz').focus(); true")
        .await?;

    // Shrinking from the desktop layout hides the persistent sidebar. The
    // focus must move to the mobile nav toggle before aria-hidden/inert is
    // applied; otherwise Chromium warns about hiding the focused button.
    h.set_viewport(390, 844).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;

    let messages = drain_console(&mut console, &mut exceptions).await;
    if messages
        .iter()
        .any(|message| message.contains("aria-hidden") && message.contains("retained focus"))
    {
        return Err(Error::message(format!(
            "narrowing the viewport emitted an aria-hidden focus warning: {messages:?}"
        )));
    }

    let focused: String = h.eval("document.activeElement?.id ?? ''").await?;
    if focused != "nav-toggle" {
        return Err(Error::message(format!(
            "narrowing the viewport should move focus to nav-toggle, got {focused:?}"
        )));
    }

    Ok(())
}
