//! Card editor (Phase 4C): prompt/solution textareas with a live
//! Markdown preview side by side and a 5 s autosave draft.
//!
//! One component covers both modes: editing an existing card (opened
//! from a Groom row's Edit button, or by recovering a draft of one) and
//! drafting a new card (the "Add card" tab, or a recovered new-card
//! draft). New-card mode deliberately keeps the ids and behavior of the
//! old Add card form (`new-prompt`, `new-solution`, `create-card`,
//! `add-card-confirmation`): the existing e2e suite drives exactly that
//! flow, so a successful create stays on the editor, clears the form and
//! shows the confirmation instead of navigating away. Saving an
//! *existing* card closes the editor back to Groom (the server's PATCH
//! already deletes the draft). Cancel deletes the draft explicitly
//! before closing — like the reference app's Cancel/Abandon — so a
//! deliberately cancelled session never triggers the recovery banner.
//!
//! Autosave is a port of the old `useAutoSave`: a 5 s interval, skipping
//! ticks while a write is in flight, while the content matches the last
//! saved/loaded baseline, or while both fields are empty. The interval
//! handle is cleared in `on_cleanup`, so leaving the editor (Save,
//! Cancel, tab switch) stops it. A subtle indicator shows "unsaved
//! changes" while dirty and "draft saved HH:MM:SS" after a write.

use flasher_types::{AutoSaveResponse, CardResponse};
use leptos::prelude::*;

use crate::api;
use crate::markdown::MarkdownView;

/// Autosave cadence, ported from the old `useAutoSave` (5 s).
#[cfg(feature = "csr")]
const AUTOSAVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

/// What the editor is working on.
#[derive(Clone, Debug)]
pub struct EditTarget {
    /// `Some(id)` edits that existing card; `None` drafts a new one.
    card_id: Option<String>,
    /// Initial textarea content (card content, or the recovered draft).
    initial_prompt: String,
    initial_solution: String,
}

impl EditTarget {
    /// A blank new-card draft (the "Add card" tab).
    pub fn new_card() -> Self {
        Self {
            card_id: None,
            initial_prompt: String::new(),
            initial_solution: String::new(),
        }
    }

    /// Edit an existing card, pre-filled with its content.
    pub fn edit(card: &CardResponse) -> Self {
        Self {
            card_id: Some(card.id.clone()),
            initial_prompt: card.prompt.clone(),
            initial_solution: card.solution.clone(),
        }
    }

    /// Recover a draft: edit mode when the draft belongs to a card that
    /// still exists, new-card mode otherwise (draft for a new card, or
    /// the card was deleted meanwhile — the draft text is kept either
    /// way).
    pub fn from_draft(draft: &AutoSaveResponse, card_still_exists: bool) -> Self {
        Self {
            card_id: draft.card_id.clone().filter(|_| card_still_exists),
            initial_prompt: draft.prompt.clone(),
            initial_solution: draft.solution.clone(),
        }
    }
}

/// How an editing session ended.
#[derive(Clone, Copy, Debug)]
pub enum CloseOutcome {
    /// An existing card was saved (navigate back to Groom).
    Saved,
    /// Closed without saving; the autosave draft was deleted first,
    /// matching the reference app's Cancel/Abandon.
    Cancelled,
}

/// The card editor: inputs left, live preview right (stacked on mobile).
// The autosave loop, save handler and the two view panes make this long;
// splitting them further would only add indirection (same reasoning as
// the groom tab).
#[allow(clippy::too_many_lines)]
#[component]
pub fn Editor(target: EditTarget, on_close: Callback<CloseOutcome>) -> impl IntoView {
    let EditTarget {
        card_id,
        initial_prompt,
        initial_solution,
    } = target;
    let is_new = card_id.is_none();

    let prompt = RwSignal::new(initial_prompt.clone());
    let solution = RwSignal::new(initial_solution.clone());
    // What the content is compared against: the loaded content at open,
    // then the last successfully autosaved content. Differences against
    // it are what "dirty" (and the 5 s PUT) means.
    let baseline = RwSignal::new((initial_prompt, initial_solution));
    // Formatted time of the last successful draft write.
    let saved_at = RwSignal::new(None::<String>);
    let validation = RwSignal::new(None::<String>);
    let error = RwSignal::new(None::<String>);
    let confirmation = RwSignal::new(false);
    // Re-entrancy guard shared by the autosave tick, Save and Cancel
    // (the old hook's `isSaving` ref): while a write is in flight the
    // tick skips and all three buttons are disabled. This closes two
    // races: an in-flight autosave PUT landing after Save's server-side
    // draft deletion would resurrect a stale draft, and a double-click
    // on Create would create duplicate cards.
    let busy = RwSignal::new(false);

    // The 5 s autosave loop (browser only). The handle lives until the
    // component unmounts — Save, Cancel and tab switches all unmount it.
    #[cfg(feature = "csr")]
    {
        let tick_card_id = card_id.clone();
        if let Ok(handle) = set_interval_with_handle(
            move || {
                if busy.get_untracked() {
                    return;
                }
                let p = prompt.get_untracked();
                let s = solution.get_untracked();
                if p.trim().is_empty() && s.trim().is_empty() {
                    return;
                }
                if baseline.with_untracked(|(bp, bs)| p == *bp && s == *bs) {
                    return;
                }
                busy.set(true);
                let tick_card_id = tick_card_id.clone();
                leptos::task::spawn_local(async move {
                    // A failed write just stays dirty and retries on the
                    // next tick (the old app behaved the same).
                    if let Ok(saved) = api::put_autosave(tick_card_id.as_deref(), &p, &s).await {
                        baseline.set((p, s));
                        saved_at.set(Some(format_hms(saved.updated_at)));
                    }
                    busy.set(false);
                });
            },
            AUTOSAVE_INTERVAL,
        ) {
            on_cleanup(move || handle.clear());
        }
    }

    let save_card_id = card_id.clone();
    let save = move |_| {
        // Honor the busy guard (see its declaration): a click while an
        // autosave PUT is in flight must not start Save — the late PUT
        // would resurrect the draft Save deletes server-side — and a
        // double-click must not create the card twice.
        if busy.get_untracked() {
            return;
        }
        validation.set(None);
        error.set(None);
        confirmation.set(false);
        let prompt_text = prompt.get_untracked();
        if prompt_text.trim().is_empty() {
            validation.set(Some("Prompt must not be empty.".to_owned()));
            return;
        }
        let solution_text = solution.get_untracked();
        let card_id = save_card_id.clone();
        busy.set(true);
        leptos::task::spawn_local(async move {
            let result = match &card_id {
                // PATCH deletes the draft server-side.
                Some(id) => api::update_card(id, &prompt_text, &solution_text)
                    .await
                    .map(|_| ()),
                // POST does not, so the draft is dropped explicitly.
                None => match api::create_card(&prompt_text, &solution_text).await {
                    Ok(_card) => api::delete_autosave().await,
                    Err(err) => Err(err),
                },
            };
            match result {
                Ok(()) if card_id.is_some() => {
                    // Move the baseline to the just-saved content BEFORE
                    // releasing busy, so a tick that fires before the
                    // unmount sees "not dirty" and cannot re-PUT the
                    // draft the PATCH just deleted server-side.
                    baseline.set((prompt_text, solution_text));
                    busy.set(false);
                    on_close.run(CloseOutcome::Saved);
                }
                Ok(()) => {
                    // New-card mode stays open (old Add card behavior):
                    // clear the form for the next card and confirm.
                    prompt.set(String::new());
                    solution.set(String::new());
                    baseline.set((String::new(), String::new()));
                    saved_at.set(None);
                    busy.set(false);
                    confirmation.set(true);
                }
                Err(err) => {
                    busy.set(false);
                    error.set(Some(err));
                }
            }
        });
    };

    // Cancel abandons the session AND deletes the autosave draft — this
    // matches the reference app (GroomView/QuizView Cancel/Abandon called
    // AutoSave.delete), so a deliberately cancelled session must not
    // produce a recovery banner on the next load. Best-effort: the
    // editor closes regardless of the DELETE result.
    let cancel = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            _ = api::delete_autosave().await;
            busy.set(false);
            on_close.run(CloseOutcome::Cancelled);
        });
    };

    // New-card mode keeps the old Add card ids: the existing e2e suite
    // drives them (see module docs).
    let (prompt_id, solution_id, save_id, validation_id, error_id, heading, save_label) = if is_new
    {
        (
            "new-prompt",
            "new-solution",
            "create-card",
            "add-card-validation",
            "add-card-error",
            "New card",
            "Create card",
        )
    } else {
        (
            "editor-prompt",
            "editor-solution",
            "editor-save",
            "editor-validation",
            "editor-error",
            "Edit card",
            "Save",
        )
    };

    let indicator = move || {
        let dirty = (prompt.get(), solution.get()) != baseline.get();
        if dirty {
            "unsaved changes".to_owned()
        } else {
            saved_at
                .get()
                .map_or(String::new(), |at| format!("draft saved {at}"))
        }
    };

    view! {
        <section class="editor">
            <h2 id="editor-heading">{heading}</h2>
            <div class="editor-panes">
                <div class="editor-inputs">
                    <label for=prompt_id>"Prompt"</label>
                    <textarea
                        id=prompt_id
                        rows="6"
                        placeholder="Front of the card (Markdown)"
                        bind:value=prompt
                    ></textarea>
                    <label for=solution_id>"Solution"</label>
                    <textarea
                        id=solution_id
                        rows="10"
                        placeholder="Back of the card (Markdown)"
                        bind:value=solution
                    ></textarea>
                    <div class="editor-bar">
                        <span class="draft-indicator" id="draft-indicator">{indicator}</span>
                        <button
                            type="button"
                            id=save_id
                            class="primary"
                            disabled=move || busy.get()
                            on:click=save
                        >
                            {save_label}
                        </button>
                        <button
                            type="button"
                            id="editor-cancel"
                            disabled=move || busy.get()
                            on:click=cancel
                        >
                            "Cancel"
                        </button>
                    </div>
                </div>
                <div class="editor-preview" id="editor-preview">
                    <MarkdownView
                        markdown=Signal::derive(move || prompt.get())
                        id="editor-preview-prompt"
                    />
                    <MarkdownView
                        markdown=Signal::derive(move || solution.get())
                        id="editor-preview-solution"
                        class="solution"
                    />
                </div>
            </div>
            {move || validation.get().map(|msg| view! {
                <p class="form-error" id=validation_id>{msg}</p>
            })}
            {move || confirmation.get().then(|| view! {
                <p class="form-ok" id="add-card-confirmation">
                    "Card created — it starts with the Disabled label; enable it in the Groom tab."
                </p>
            })}
            {move || error.get().map(|err| view! {
                <p class="form-error" id=error_id>"Something went wrong: " {err}</p>
            })}
        </section>
    }
}

/// Local `HH:MM:SS` for the draft indicator (browser clock).
#[cfg(feature = "csr")]
fn format_hms(epoch_ms: i64) -> String {
    // Unix epoch millis fit an f64 exactly until far past any relevant
    // date; the sub-millisecond truncation is invisible on a clock label.
    #[allow(clippy::cast_precision_loss)]
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(epoch_ms as f64));
    format!(
        "{:02}:{:02}:{:02}",
        date.get_hours(),
        date.get_minutes(),
        date.get_seconds()
    )
}
