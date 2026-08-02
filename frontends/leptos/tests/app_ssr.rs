//! Host-target smoke test: `App` renders server-side and contains the
//! heading plus the initial states of the tabs. Run with
//! `cargo test --no-default-features --features ssr`.
//!
//! No fetch happens here: browser-only API calls live inside effects and
//! event handlers, which never run during server-side rendering.

#![cfg(feature = "ssr")]
// The editor's view tree (textareas + label picker) is deep enough that
// tachys' type-level layout computation needs more than the default
// query depth when instantiated in these tests.
#![recursion_limit = "256"]

use flasher_leptos::{
    AccountTab, App, AuthScreenTab, EditTarget, EditorCloseOutcome, EditorTab, GroomTab,
    LabelManagerTab, QuizTab,
};
use leptos::prelude::*;

#[test]
fn app_renders_flasher_heading() {
    let html = view! { <App/> }.to_html();
    assert!(html.contains("Flasher"), "rendered html: {html}");
}

#[test]
fn app_renders_session_check_splash_initially() {
    // The session probe only runs under csr, so the ssr render stays in
    // the checking state: no tabs, no auth form yet.
    let html = view! { <App/> }.to_html();
    assert!(html.contains("auth-checking"), "rendered html: {html}");
    assert!(!html.contains("tab-quiz"), "rendered html: {html}");
}

#[test]
fn auth_screen_renders_its_loading_state() {
    // The bootstrap fetch only runs under csr, so the initial render is
    // the loading placeholder inside the centered card.
    let html = view! { <AuthScreenTab on_login=Callback::new(|_: String| {}) /> }.to_html();
    assert!(html.contains("auth-screen"), "rendered html: {html}");
    assert!(html.contains("auth-loading"), "rendered html: {html}");
    assert!(html.contains("Flasher"), "rendered html: {html}");
}

#[test]
fn account_tab_renders_its_initial_state() {
    // The passkey fetch only runs under csr: username visible, passkey
    // list loading.
    let html = view! {
        <AccountTab
            username="kakimena".to_owned()
            dev_mode=false
            on_logout=Callback::new(|(): ()| {})
        />
    }
    .to_html();
    assert!(html.contains("kakimena"), "rendered html: {html}");
    assert!(html.contains("passkeys-loading"), "rendered html: {html}");
    assert!(html.contains("add-passkey"), "rendered html: {html}");
    // Auth mode: the logout button is there.
    assert!(html.contains("logout"), "rendered html: {html}");
}

#[test]
fn account_tab_hides_logout_in_dev_mode() {
    // Dev bypass has no session to end: no logout button, but the
    // passkey management card is still offered (the endpoints attach to
    // the dev user).
    let html = view! {
        <AccountTab
            username="e2e".to_owned()
            dev_mode=true
            on_logout=Callback::new(|(): ()| {})
        />
    }
    .to_html();
    assert!(!html.contains("id=\"logout\""), "rendered html: {html}");
    assert!(html.contains("passkeys-card"), "rendered html: {html}");
}

#[test]
fn app_renders_no_recovery_banner_initially() {
    // The draft check only runs under csr, so the ssr render has no
    // recovery banner.
    let html = view! { <App/> }.to_html();
    assert!(!html.contains("recovery-banner"), "rendered html: {html}");
}

#[test]
fn editor_new_card_mode_keeps_add_card_ids() {
    // New-card mode is the Add card tab; it keeps the old form ids (the
    // existing browser tests drive them).
    let html = view! {
        <EditorTab
            target=EditTarget::new_card()
            on_close=Callback::new(|_: EditorCloseOutcome| {})
        />
    }
    .to_html();
    assert!(html.contains("new-prompt"), "rendered html: {html}");
    assert!(html.contains("new-solution"), "rendered html: {html}");
    assert!(html.contains("create-card"), "rendered html: {html}");
    assert!(html.contains("New card"), "rendered html: {html}");
    assert!(
        html.contains("editor-preview-prompt"),
        "rendered html: {html}"
    );
    assert!(
        html.contains("editor-preview-solution"),
        "rendered html: {html}"
    );
}

#[test]
fn editor_edit_mode_prefills_from_draft() {
    // A recovered draft of an existing card opens in edit mode with the
    // draft text pre-filled.
    let draft = flasher_types::AutoSaveResponse {
        card_id: Some("card-1".to_owned()),
        prompt: "Draft prompt".to_owned(),
        solution: "Draft solution".to_owned(),
        updated_at: 1_000,
    };
    let html = view! {
        <EditorTab
            target=EditTarget::from_draft(&draft, true)
            on_close=Callback::new(|_: EditorCloseOutcome| {})
        />
    }
    .to_html();
    assert!(html.contains("editor-prompt"), "rendered html: {html}");
    assert!(html.contains("editor-solution"), "rendered html: {html}");
    assert!(html.contains("editor-save"), "rendered html: {html}");
    assert!(html.contains("Edit card"), "rendered html: {html}");
    assert!(html.contains("Draft prompt"), "rendered html: {html}");
    assert!(html.contains("Draft solution"), "rendered html: {html}");
}

#[test]
fn editor_falls_back_to_new_mode_for_deleted_card() {
    // Same draft, but the card is gone: new-card mode with the draft
    // text kept.
    let draft = flasher_types::AutoSaveResponse {
        card_id: Some("card-gone".to_owned()),
        prompt: "Draft prompt".to_owned(),
        solution: String::new(),
        updated_at: 1_000,
    };
    let target = EditTarget::from_draft(&draft, false);
    let html = view! {
        <EditorTab
            target=target
            on_close=Callback::new(|_: EditorCloseOutcome| {})
        />
    }
    .to_html();
    assert!(html.contains("new-prompt"), "rendered html: {html}");
    assert!(html.contains("New card"), "rendered html: {html}");
    assert!(html.contains("Draft prompt"), "rendered html: {html}");
}

#[test]
fn quiz_tab_renders_its_initial_state() {
    // The next-card fetch only runs under csr, so the ssr render stays
    // in the loading state.
    let html = view! { <QuizTab/> }.to_html();
    assert!(html.contains("quiz-loading"), "rendered html: {html}");
}

#[test]
fn groom_tab_renders_its_initial_state() {
    // Rendering the component directly exercises the Groom initial state
    // without a browser: search input, the label filter button (labels
    // dissolved the status dropdown, owner decision 2026-08-01) plus the
    // loading placeholder (the fetch effect only runs under csr).
    let html = view! { <GroomTab on_edit=Callback::new(|_| {}) /> }.to_html();
    assert!(html.contains("groom-search"), "rendered html: {html}");
    assert!(
        html.contains("groom-label-filter-button"),
        "rendered html: {html}"
    );
    assert!(html.contains("Loading cards"), "rendered html: {html}");
}

#[test]
fn labels_page_renders_its_initial_state() {
    let html = view! { <LabelManagerTab/> }.to_html();
    assert!(html.contains("labels-page"), "rendered html: {html}");
    assert!(html.contains("new-label-name"), "rendered html: {html}");
    assert!(html.contains("labels-loading"), "rendered html: {html}");
}
