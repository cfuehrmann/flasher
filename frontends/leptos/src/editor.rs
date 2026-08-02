//! The shared card editor for new cards and existing cards.
//!
//! The two workflows deliberately have different server-side draft stores:
//! one new-card draft per user and one edit draft per user/card. The editor
//! looks the same in both modes, but a draft can never cross from one mode to
//! the other. Drafts are autosaved after a short idle debounce and also at a
//! maximum interval while the user keeps typing.

use std::time::Duration;

#[cfg(feature = "csr")]
use flasher_types::{CardEditDraftResponse, NewCardDraftResponse};
use flasher_types::{CardResponse, LabelResponse};
use leptos::prelude::*;

use crate::api;
use crate::labels::toggle_label_name;
use crate::markdown::MarkdownView;

#[cfg_attr(not(feature = "csr"), allow(dead_code))]
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(5);
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
const AUTOSAVE_IDLE: Duration = Duration::from_millis(750);

/// What the editor is working on.
#[derive(Clone, Debug)]
pub struct EditTarget {
    /// `Some(id)` edits that existing card; `None` drafts a new one.
    pub(crate) card_id: Option<String>,
    pub(crate) initial_prompt: String,
    pub(crate) initial_solution: String,
    pub(crate) initial_labels: Vec<String>,
    pub(crate) initial_revision: i64,
}

impl EditTarget {
    /// A blank new-card editor.
    pub fn new_card() -> Self {
        Self {
            card_id: None,
            initial_prompt: String::new(),
            initial_solution: String::new(),
            initial_labels: Vec::new(),
            initial_revision: 0,
        }
    }

    /// An existing card's persisted content.
    pub fn edit(card: &CardResponse) -> Self {
        Self {
            card_id: Some(card.id.clone()),
            initial_prompt: card.prompt.clone(),
            initial_solution: card.solution.clone(),
            initial_labels: card.labels.clone(),
            initial_revision: card.revision,
        }
    }
}

/// How an editing session ended.
#[derive(Clone, Copy, Debug)]
pub enum CloseOutcome {
    /// An existing card was committed.
    Saved,
    /// The editor was closed while retaining its draft.
    Closed,
    /// The matching draft was explicitly deleted.
    Discarded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DraftFields {
    prompt: String,
    solution: String,
    labels: Vec<String>,
}

fn autosave_fields_differ(current: &DraftFields, baseline: &DraftFields, is_new: bool) -> bool {
    if is_new {
        current.prompt != baseline.prompt || current.solution != baseline.solution
    } else {
        current != baseline
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DraftStatus {
    Loading,
    Clean,
    Dirty,
    Saving,
    Saved(String),
    Error(String),
}

/// The shared card editor: prompt, solution, labels, and live previews.
#[allow(clippy::too_many_lines)]
#[component]
pub fn Editor(
    target: EditTarget,
    on_close: Callback<CloseOutcome>,
    #[prop(optional)] on_busy_change: Option<Callback<bool>>,
    #[prop(optional)] on_dirty_change: Option<Callback<bool>>,
) -> impl IntoView {
    let EditTarget {
        card_id,
        initial_prompt,
        initial_solution,
        initial_labels,
        initial_revision,
    } = target;
    let is_new = card_id.is_none();
    let prompt = RwSignal::new(initial_prompt.clone());
    let solution = RwSignal::new(initial_solution.clone());
    let working_labels = RwSignal::new(initial_labels.clone());
    let all_labels = RwSignal::new(Vec::<LabelResponse>::new());
    let baseline = RwSignal::new(DraftFields {
        prompt: initial_prompt,
        solution: initial_solution,
        labels: initial_labels,
    });
    let revision = RwSignal::new(initial_revision);
    let draft_loaded = RwSignal::new(!cfg!(feature = "csr"));
    let status = RwSignal::new(if cfg!(feature = "csr") {
        DraftStatus::Loading
    } else {
        DraftStatus::Clean
    });
    let busy = RwSignal::new(false);
    let confirmation = RwSignal::new(false);
    let change_generation = RwSignal::new(0_u64);
    let retry_requested = RwSignal::new(false);
    let close_requested = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let busy_change = on_busy_change.clone();
    let set_busy = Callback::new(move |value: bool| {
        if let Some(callback) = &busy_change {
            callback.run(value);
        }
        busy.set(value);
    });
    let dirty_change = on_dirty_change.clone();
    let set_dirty = Callback::new(move |value: bool| {
        if let Some(callback) = &dirty_change {
            callback.run(value);
        }
    });
    let busy_change = on_busy_change.clone();
    let dirty_change = on_dirty_change.clone();
    on_cleanup(move || {
        if let Some(callback) = &busy_change {
            callback.run(false);
        }
        if let Some(callback) = &dirty_change {
            callback.run(false);
        }
    });

    // Every editor instance loads only its own workflow's draft. This also
    // handles tab switches, browser reloads, session expiry/re-login, and
    // multiple browser tabs without any browser-side sensitive storage.
    #[cfg(feature = "csr")]
    {
        let load_card_id = card_id.clone();
        leptos::task::spawn_local(async move {
            let labels_result = api::labels().await;
            let draft_result = match load_card_id.as_deref() {
                Some(id) => api::get_card_edit_draft(id)
                    .await
                    .map(|draft| draft.map(EditDraftLoaded::Existing)),
                None => api::get_new_card_draft()
                    .await
                    .map(|draft| draft.map(EditDraftLoaded::New)),
            };
            if let Ok(labels) = labels_result {
                all_labels.set(labels);
            }
            match draft_result {
                Ok(Some(EditDraftLoaded::New(draft))) => {
                    let fields = DraftFields {
                        prompt: draft.prompt,
                        solution: draft.solution,
                        labels: working_labels.get_untracked(),
                    };
                    prompt.set(fields.prompt.clone());
                    solution.set(fields.solution.clone());
                    working_labels.set(fields.labels.clone());
                    baseline.set(fields);
                    status.set(DraftStatus::Saved(format_hms(draft.updated_at)));
                }
                Ok(Some(EditDraftLoaded::Existing(draft))) => {
                    let fields = DraftFields {
                        prompt: draft.prompt,
                        solution: draft.solution,
                        labels: draft.labels,
                    };
                    prompt.set(fields.prompt.clone());
                    solution.set(fields.solution.clone());
                    working_labels.set(fields.labels.clone());
                    baseline.set(fields);
                    revision.set(draft.base_revision);
                    status.set(DraftStatus::Saved(format_hms(draft.updated_at)));
                }
                Ok(None) => status.set(DraftStatus::Clean),
                Err(err) => {
                    error.set(Some(err));
                    status.set(DraftStatus::Error("draft could not be loaded".to_owned()));
                }
            }
            draft_loaded.set(true);
        });
    }

    // The actual autosave operation is shared by the idle debounce and the
    // maximum-interval timer. Existing-card drafts write a complete snapshot;
    // new-card drafts deliberately persist only prompt and solution because
    // labels are committed only by Create.
    let autosave = Callback::new({
        let card_id = card_id.clone();
        let set_busy = set_busy.clone();
        let set_dirty = set_dirty.clone();
        let on_close = on_close.clone();
        move |(): ()| {
            if !draft_loaded.get_untracked() {
                return;
            }
            if busy.get_untracked() {
                // An input can arrive while the previous request is still
                // in flight. Remember it instead of silently treating that
                // snapshot as saved; the effect below retries immediately
                // after the request finishes.
                retry_requested.set(true);
                return;
            }
            let fields = DraftFields {
                prompt: prompt.get_untracked(),
                solution: solution.get_untracked(),
                labels: working_labels.get_untracked(),
            };
            if !autosave_fields_differ(&fields, &baseline.get_untracked(), is_new) {
                retry_requested.set(false);
                set_dirty.run(false);
                if close_requested.get_untracked() {
                    close_requested.set(false);
                    on_close.run(CloseOutcome::Closed);
                }
                return;
            }
            set_busy.run(true);
            status.set(DraftStatus::Saving);
            let card_id = card_id.clone();
            let base_revision = revision.get_untracked();
            let set_busy_for_request = set_busy.clone();
            leptos::task::spawn_local(async move {
                let result = match card_id.as_deref() {
                    Some(id) => api::put_card_edit_draft(
                        id,
                        base_revision,
                        &fields.prompt,
                        &fields.solution,
                        &fields.labels,
                    )
                    .await
                    .map(|draft| (draft.updated_at, fields)),
                    None => api::put_new_card_draft(&fields.prompt, &fields.solution)
                        .await
                        .map(|draft| (draft.updated_at, fields)),
                };
                match result {
                    Ok((saved_at, saved_fields)) => {
                        if !autosave_fields_differ(
                            &DraftFields {
                                prompt: prompt.get_untracked(),
                                solution: solution.get_untracked(),
                                labels: working_labels.get_untracked(),
                            },
                            &saved_fields,
                            is_new,
                        ) {
                            baseline.set(saved_fields);
                            retry_requested.set(false);
                            status.set(DraftStatus::Saved(format_hms(saved_at)));
                            set_dirty.run(false);
                            let should_close = close_requested.get_untracked();
                            close_requested.set(false);
                            set_busy_for_request.run(false);
                            if should_close {
                                on_close.run(CloseOutcome::Closed);
                            }
                            return;
                        } else {
                            // The user changed the editor while this
                            // request was in flight. Keep the older snapshot
                            // as the baseline and immediately send the
                            // newer one after `busy` becomes false.
                            retry_requested.set(true);
                            status.set(DraftStatus::Dirty);
                            set_dirty.run(true);
                        }
                    }
                    Err(err) => {
                        status.set(DraftStatus::Error(err.clone()));
                        error.set(Some(err));
                        set_dirty.run(true);
                    }
                }
                set_busy_for_request.run(false);
            });
        }
    });

    // A save completion flips `busy` back to false after marking a newer
    // snapshot pending. This effect serializes the retry without allowing
    // overlapping writes to the same target draft.
    #[cfg(feature = "csr")]
    {
        let autosave = autosave.clone();
        Effect::new(move |_| {
            if retry_requested.get() && !busy.get() {
                retry_requested.set(false);
                autosave.run(());
            }
        });
    }

    // Maximum dirty interval: continuous typing cannot postpone the save
    // forever. The input handler below adds the shorter idle save.
    #[cfg(feature = "csr")]
    if let Ok(handle) = set_interval_with_handle(move || autosave.run(()), AUTOSAVE_INTERVAL) {
        on_cleanup(move || handle.clear());
    }

    let schedule_autosave = {
        let autosave = autosave.clone();
        Callback::new(move |(): ()| {
            change_generation.update(|generation| *generation += 1);
            let armed = change_generation.get_untracked();
            let fields = DraftFields {
                prompt: prompt.get_untracked(),
                solution: solution.get_untracked(),
                labels: working_labels.get_untracked(),
            };
            if autosave_fields_differ(&fields, &baseline.get_untracked(), is_new) {
                if busy.get_untracked() {
                    retry_requested.set(true);
                }
                status.set(DraftStatus::Dirty);
                set_dirty.run(true);
            }
            set_timeout(
                move || {
                    if change_generation.get_untracked() == armed {
                        autosave.run(());
                    }
                },
                AUTOSAVE_IDLE,
            );
        })
    };

    let on_text_input = move |_| schedule_autosave.run(());
    let on_label_change = move |name: String, checked: bool| {
        let next = toggle_label_name(&working_labels.get_untracked(), &name, checked);
        working_labels.set(next);
        schedule_autosave.run(());
    };

    let save = {
        let card_id = card_id.clone();
        let set_busy = set_busy.clone();
        move |_| {
            if busy.get_untracked() || !draft_loaded.get_untracked() {
                return;
            }
            error.set(None);
            confirmation.set(false);
            let prompt_text = prompt.get_untracked();
            if prompt_text.trim().is_empty() {
                error.set(Some("Prompt must not be empty.".to_owned()));
                return;
            }
            let labels = working_labels.get_untracked();
            if labels.is_empty() {
                error.set(Some("Choose at least one label.".to_owned()));
                return;
            }
            let solution_text = solution.get_untracked();
            let card_id = card_id.clone();
            let expected_revision = revision.get_untracked();
            set_busy.run(true);
            status.set(DraftStatus::Saving);
            let set_busy_for_request = set_busy.clone();
            leptos::task::spawn_local(async move {
                let result = match card_id.as_deref() {
                    Some(id) => api::save_card_edit(
                        id,
                        expected_revision,
                        &prompt_text,
                        &solution_text,
                        &labels,
                    )
                    .await
                    .map(|_| ()),
                    None => api::create_card(&prompt_text, &solution_text, &labels)
                        .await
                        .map(|_| ()),
                };
                match result {
                    Ok(()) if card_id.is_some() => {
                        set_dirty.run(false);
                        close_requested.set(false);
                        set_busy_for_request.run(false);
                        on_close.run(CloseOutcome::Saved);
                    }
                    Ok(()) => {
                        set_dirty.run(false);
                        close_requested.set(false);
                        prompt.set(String::new());
                        solution.set(String::new());
                        baseline.set(DraftFields {
                            prompt: String::new(),
                            solution: String::new(),
                            labels: labels.clone(),
                        });
                        status.set(DraftStatus::Clean);
                        set_busy_for_request.run(false);
                        confirmation.set(true);
                    }
                    Err(err) => {
                        set_dirty.run(true);
                        set_busy_for_request.run(false);
                        status.set(DraftStatus::Error(err.clone()));
                        error.set(Some(err));
                    }
                }
            });
        }
    };

    // Closing retains the draft. Only the explicit Discard button deletes
    // it, which keeps ordinary navigation warning-free and reversible.
    let close = {
        let autosave = autosave.clone();
        move |_| {
            if !draft_loaded.get_untracked() || busy.get_untracked() {
                return;
            }
            let fields = DraftFields {
                prompt: prompt.get_untracked(),
                solution: solution.get_untracked(),
                labels: working_labels.get_untracked(),
            };
            if !autosave_fields_differ(&fields, &baseline.get_untracked(), is_new) {
                set_dirty.run(false);
                on_close.run(CloseOutcome::Closed);
            } else {
                close_requested.set(true);
                autosave.run(());
            }
        }
    };
    let discard = {
        let card_id = card_id.clone();
        let set_busy = set_busy.clone();
        move |_| {
            if busy.get_untracked() {
                return;
            }
            set_busy.run(true);
            let card_id = card_id.clone();
            let set_busy_for_request = set_busy.clone();
            leptos::task::spawn_local(async move {
                let result = match card_id.as_deref() {
                    Some(id) => api::delete_card_edit_draft(id).await,
                    None => api::delete_new_card_draft().await,
                };
                match result {
                    Ok(()) => {
                        set_dirty.run(false);
                        close_requested.set(false);
                        set_busy_for_request.run(false);
                        on_close.run(CloseOutcome::Discarded);
                    }
                    Err(err) => {
                        set_dirty.run(true);
                        set_busy_for_request.run(false);
                        status.set(DraftStatus::Error(err.clone()));
                        error.set(Some(err));
                    }
                }
            });
        }
    };

    let (prompt_id, solution_id, save_id, validation_id, heading, save_label) = if is_new {
        (
            "new-prompt",
            "new-solution",
            "create-card",
            "add-card-validation",
            "New card",
            "Create card",
        )
    } else {
        (
            "editor-prompt",
            "editor-solution",
            "editor-save",
            "editor-validation",
            "Edit card",
            "Save",
        )
    };

    let indicator = move || match status.get() {
        DraftStatus::Loading => "Loading draft…".to_owned(),
        DraftStatus::Clean => "".to_owned(),
        DraftStatus::Dirty => "unsaved changes".to_owned(),
        DraftStatus::Saving => "saving draft…".to_owned(),
        DraftStatus::Saved(at) => format!("draft saved {at}"),
        DraftStatus::Error(message) => format!("draft save failed: {message}"),
    };

    view! {
        <section class="editor">
            <h2 id="editor-heading">{heading}</h2>
            <div class="editor-panes">
                <div class="editor-inputs">
                    {move || (!draft_loaded.get()).then(|| view! {
                        <p class="editor-loading" id="editor-loading">"Loading draft…"</p>
                    })}
                    <label for=prompt_id>"Prompt"</label>
                    <textarea
                        id=prompt_id
                        rows="6"
                        placeholder="Front of the card (Markdown)"
                        bind:value=prompt
                        on:input=on_text_input
                        disabled=move || !draft_loaded.get()
                    ></textarea>
                    <label for=solution_id>"Solution"</label>
                    <textarea
                        id=solution_id
                        rows="10"
                        placeholder="Back of the card (Markdown)"
                        bind:value=solution
                        on:input=on_text_input
                        disabled=move || !draft_loaded.get()
                    ></textarea>
                    <div class="editor-labels" id="editor-labels">
                        <div class="editor-label-heading">
                            <label>"Labels"</label>
                            <span class="editor-label-requirement" id="editor-label-requirement">
                                "Choose at least one label."
                            </span>
                        </div>
                        {move || {
                            all_labels
                                .get()
                                .into_iter()
                                .map(|label| {
                                    let name = label.name.clone();
                                    let box_id = format!("editor-label-{name}");
                                    let for_id = box_id.clone();
                                    let checked_name = name.clone();
                                    view! {
                                        <label class="label-filter-item" for=for_id>
                                            {label.name.clone()}
                                            <input
                                                type="checkbox"
                                                id=box_id
                                                prop:checked=move || {
                                                    working_labels.get().contains(&checked_name)
                                                }
                                                disabled=move || !draft_loaded.get()
                                                on:change={
                                                    let name = name.clone();
                                                    move |ev: leptos::ev::Event| {
                                                        on_label_change(
                                                            name.clone(),
                                                            event_target_checked(&ev),
                                                        );
                                                    }
                                                }
                                            />
                                        </label>
                                    }
                                })
                                .collect_view()
                        }}
                        {move || (draft_loaded.get() && all_labels.get().is_empty()).then(|| view! {
                            <p class="editor-label-hint">"Create labels on the Labels page first."</p>
                        })}
                    </div>
                    <div class="editor-bar">
                        <span class="draft-indicator" id="draft-indicator">{indicator}</span>
                        <button
                            type="button"
                            id=save_id
                            class="primary"
                            disabled=move || {
                                busy.get()
                                    || !draft_loaded.get()
                                    || prompt.get().trim().is_empty()
                                    || working_labels.get().is_empty()
                            }
                            on:click=save
                        >
                            {save_label}
                        </button>
                        <button
                            type="button"
                            id="editor-close"
                            disabled=move || busy.get()
                            on:click=close
                        >
                            "Close"
                        </button>
                        <button
                            type="button"
                            id="editor-discard"
                            disabled=move || busy.get()
                            on:click=discard
                        >
                            "Discard"
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
                    />
                </div>
            </div>
            {move || error.get().map(|message| view! {
                <p class="form-error" id={validation_id}>{message}</p>
            })}
            {move || confirmation.get().then(|| view! {
                <p class="form-ok" id="add-card-confirmation">"Card created."</p>
            })}
        </section>
    }
}

#[cfg(feature = "csr")]
enum EditDraftLoaded {
    New(NewCardDraftResponse),
    Existing(CardEditDraftResponse),
}

/// Local `HH:MM:SS` for the draft indicator.
#[cfg(feature = "csr")]
fn format_hms(epoch_ms: i64) -> String {
    #[allow(clippy::cast_precision_loss)]
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(epoch_ms as f64));
    format!(
        "{:02}:{:02}:{:02}",
        date.get_hours(),
        date.get_minutes(),
        date.get_seconds()
    )
}

#[cfg(not(feature = "csr"))]
fn format_hms(_epoch_ms: i64) -> String {
    String::new()
}
