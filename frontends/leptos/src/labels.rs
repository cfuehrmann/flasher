//! Shared label filter for the groom and quiz tabs (owner decision
//! 2026-08-01): a quiet "Labels (n/m) ▾" button opening a checkbox
//! panel — union semantics (a card matches when it carries ANY selected
//! label). The selection is owned by the parent and persisted per page
//! in `localStorage`, so the groom and quiz selections are independent.
//! Selection identity is the stable label ID; names are resolved only for
//! the existing card-filter wire format and for display.

use flasher_types::LabelResponse;
use leptos::prelude::*;

/// One entry in a persisted filter selection. The `Name` variant is a
/// one-time compatibility bridge for selections written by the pre-ID UI;
/// new writes always use `Id` so a rename cannot invalidate the selection.
#[derive(Clone, Debug, PartialEq)]
pub enum StoredLabelSelection {
    Id(i64),
    Name(String),
}

/// Joins a label selection for the wire and `localStorage`. (The seed
/// names contain no comma; creation with arbitrary names is future work
/// and will keep the constraint.)
pub fn join_labels(selected: &[String]) -> String {
    selected.join(",")
}

/// Serializes a stable-ID selection for localStorage.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
pub fn join_label_ids(selected: &[i64]) -> String {
    selected
        .iter()
        .map(|id| format!("id:{id}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Splits a persisted selection. Bare entries are legacy names; the
/// explicit `id:` prefix makes numeric label names unambiguous.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
pub fn split_stored_labels(raw: &str) -> Vec<StoredLabelSelection> {
    raw.split(',')
        .filter(|name| !name.is_empty())
        .map(|entry| {
            entry
                .strip_prefix("id:")
                .and_then(|id| id.parse::<i64>().ok())
                .map_or_else(
                    || StoredLabelSelection::Name(entry.to_owned()),
                    StoredLabelSelection::Id,
                )
        })
        .collect()
}

/// Resolves persisted entries against the current label list and returns
/// stable IDs. Unknown IDs/names are dropped (for example after deletion).
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
pub fn resolve_stored_labels(
    stored: &[StoredLabelSelection],
    labels: &[LabelResponse],
) -> Vec<i64> {
    stored
        .iter()
        .filter_map(|entry| match entry {
            StoredLabelSelection::Id(id) if labels.iter().any(|label| label.id == *id) => Some(*id),
            StoredLabelSelection::Name(name) => labels
                .iter()
                .find(|label| label.name == *name)
                .map(|label| label.id),
            StoredLabelSelection::Id(_) => None,
        })
        .fold(Vec::new(), |mut ids, id| {
            if !ids.contains(&id) {
                ids.push(id);
            }
            ids
        })
}

/// Converts stable IDs to the names expected by the existing card-filter
/// API. Names are a wire representation only; selection identity remains ID.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
pub fn selected_label_names(labels: &[LabelResponse], selected: &[i64]) -> Vec<String> {
    selected
        .iter()
        .filter_map(|id| {
            labels
                .iter()
                .find(|label| label.id == *id)
                .map(|label| label.name.clone())
        })
        .collect()
}

/// Upgrades a legacy name-based selection when a label is renamed while the
/// user is on the management page. This preserves the old selection across
/// the remounted Quiz and Groom components; all other legacy names are
/// resolved to IDs when those pages next load their label list.
#[cfg(feature = "csr")]
pub fn preserve_selection_after_rename(old_name: &str, id: i64) {
    for key in ["flasher-quiz-labels", "flasher-groom-labels"] {
        let Some(raw) = storage_get(key) else {
            continue;
        };
        let stored = split_stored_labels(&raw);
        let mut changed = false;
        let upgraded = stored
            .into_iter()
            .map(|entry| match entry {
                StoredLabelSelection::Name(name) if name == old_name => {
                    changed = true;
                    StoredLabelSelection::Id(id)
                }
                other => other,
            })
            .collect::<Vec<_>>();
        if changed {
            let serialized = upgraded
                .into_iter()
                .map(|entry| match entry {
                    StoredLabelSelection::Id(id) => format!("id:{id}"),
                    StoredLabelSelection::Name(name) => name,
                })
                .collect::<Vec<_>>()
                .join(",");
            storage_set(key, &serialized);
        }
    }
}

/// Adds or removes `name` from a label set (a checkbox toggle). Shared
/// by the filter panel and the per-card label editor. (Only csr code
/// calls this.)
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
pub fn toggle_label_name(names: &[String], name: &str, checked: bool) -> Vec<String> {
    let mut next = names.to_vec();
    if checked {
        if !next.iter().any(|n| n == name) {
            next.push(name.to_owned());
        }
    } else {
        next.retain(|n| n != name);
    }
    next
}

/// Adds or removes a stable label ID from a filter selection.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
pub fn toggle_label_id(ids: &[i64], id: i64, checked: bool) -> Vec<i64> {
    let mut next = ids.to_vec();
    if checked {
        if !next.contains(&id) {
            next.push(id);
        }
    } else {
        next.retain(|existing| *existing != id);
    }
    next
}

/// Reads a `localStorage` value; any storage failure (private mode,
/// disabled storage) yields `None` — persistence is a convenience, never
/// fatal.
#[cfg(feature = "csr")]
pub fn storage_get(key: &str) -> Option<String> {
    leptos::prelude::window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|storage| storage.get_item(key).ok())
        .flatten()
}

/// Writes a `localStorage` value, ignoring storage failures.
#[cfg(feature = "csr")]
pub fn storage_set(key: &str, value: &str) {
    if let Ok(Some(storage)) = leptos::prelude::window().local_storage() {
        let _ = storage.set_item(key, value);
    }
}

/// Removes a `localStorage` value, ignoring storage failures. Used for
/// one-time migrations of superseded keys.
#[cfg(feature = "csr")]
pub fn storage_remove(key: &str) {
    if let Ok(Some(storage)) = leptos::prelude::window().local_storage() {
        let _ = storage.remove_item(key);
    }
}

#[cfg(test)]
mod tests {
    use flasher_types::LabelResponse;

    use super::{
        StoredLabelSelection, join_label_ids, join_labels, resolve_stored_labels,
        selected_label_names, split_stored_labels, toggle_label_id, toggle_label_name,
    };

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn toggle_label_name_adds_and_removes() {
        let start = names(&["Enabled"]);
        // Add a new name.
        assert_eq!(
            toggle_label_name(&start, "Disabled", true),
            names(&["Enabled", "Disabled"])
        );
        // Adding an existing name is a no-op (no duplicates).
        assert_eq!(toggle_label_name(&start, "Enabled", true), start);
        // Remove a member; removing an absent name is a no-op.
        assert_eq!(toggle_label_name(&start, "Enabled", false), names(&[]));
        assert_eq!(toggle_label_name(&start, "Nope", false), start);
    }

    #[test]
    fn persisted_filter_selection_resolves_by_id_and_current_name() {
        let labels = vec![
            LabelResponse {
                id: 7,
                name: "aaa".to_owned(),
            },
            LabelResponse {
                id: 8,
                name: "Other".to_owned(),
            },
        ];
        assert_eq!(
            resolve_stored_labels(&split_stored_labels("id:7"), &labels),
            vec![7]
        );
        assert_eq!(
            resolve_stored_labels(&split_stored_labels("xxx"), &labels),
            Vec::<i64>::new()
        );
        assert_eq!(
            resolve_stored_labels(&split_stored_labels("id:999,id:7"), &labels),
            vec![7]
        );
        assert_eq!(
            resolve_stored_labels(
                &[
                    StoredLabelSelection::Id(7),
                    StoredLabelSelection::Id(7),
                    StoredLabelSelection::Name("aaa".to_owned()),
                ],
                &labels,
            ),
            vec![7]
        );
        assert_eq!(selected_label_names(&labels, &[7]), vec!["aaa".to_owned()]);
        assert_eq!(toggle_label_id(&[7], 8, true), vec![7, 8]);
        assert_eq!(toggle_label_id(&[7, 8], 7, false), vec![8]);
    }

    #[test]
    fn filter_selections_have_explicit_wire_encodings() {
        assert_eq!(join_labels(&names(&["aaa", "Other"])), "aaa,Other");
        assert_eq!(join_label_ids(&[7, 8]), "id:7,id:8");
        assert_eq!(
            split_stored_labels("id:7,legacy,id:not-a-number"),
            vec![
                StoredLabelSelection::Id(7),
                StoredLabelSelection::Name("legacy".to_owned()),
                StoredLabelSelection::Name("id:not-a-number".to_owned()),
            ]
        );
    }
}

/// The multi-select label filter: a "Labels ▾" button that opens a
/// checkbox panel (the ⋯-menu backdrop idiom: a transparent
/// full-viewport layer swallows the closing click, so no window
/// listeners are needed), with the selected labels shown as badges to
/// the button's right (owner feedback 2026-08-01: the selection should
/// be visible without opening the panel; no counts on the button).
/// Sits on its own row, left-aligned, directly under the top menu.
#[component]
pub fn LabelFilter(
    /// All labels of the user (a signal — the list is fetched after
    /// mount, and plain props would freeze the initial empty vec).
    labels: RwSignal<Vec<LabelResponse>>,
    /// The selected stable label IDs (owned by the parent; updated via
    /// `on_change`).
    selected: RwSignal<Vec<i64>>,
    /// Called with the new selection on every checkbox change.
    on_change: Callback<Vec<i64>>,
    /// DOM id prefix (distinguishes the groom and quiz filters in e2e).
    id_prefix: &'static str,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let button_id = format!("{id_prefix}-label-filter-button");
    let panel_id = format!("{id_prefix}-label-filter-panel");
    view! {
        <div class="label-filter">
            <button
                type="button"
                class="label-filter-button"
                id=button_id
                aria-expanded=move || open.get().to_string()
                on:click=move |_| open.update(|o| *o = !*o)
            >
                "Labels ▾"
            </button>
            {move || {
                selected
                    .get()
                    .into_iter()
                    .filter_map(|id| {
                        labels
                            .get()
                            .into_iter()
                            .find(|label| label.id == id)
                    })
                    .map(|label| {
                        let name = label.name.clone();
                        let badge_id = format!("{id_prefix}-selected-{name}");
                        view! {
                            <span class="badge label" id=badge_id>
                                {name}
                            </span>
                        }
                    })
                    .collect_view()
            }}
            {move || {
                open.get().then(|| {
                    let panel_id = panel_id.clone();
                    view! {
                        <div class="label-filter-backdrop" on:click=move |_| open.set(false)></div>
                        <div class="label-filter-panel" id=panel_id>
                            {labels
                                .get()
                                .into_iter()
                                .map(|label| {
                                    let label_id = label.id;
                                    let name = label.name.clone();
                                    let box_id = format!("{id_prefix}-label-{name}");
                                    let on_toggle = {
                                        move |ev: leptos::ev::Event| {
                                            let next = toggle_label_id(
                                                &selected.get_untracked(),
                                                label_id,
                                                event_target_checked(&ev),
                                            );
                                            on_change.run(next);
                                        }
                                    };
                                    let for_id = box_id.clone();
                                    view! {
                                        <label class="label-filter-item" for=for_id>
                                            {label.name.clone()}
                                            <input
                                                type="checkbox"
                                                id=box_id
                                                prop:checked=move || selected.get().contains(&label_id)
                                                on:change=on_toggle
                                            />
                                        </label>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                })
            }}
        </div>
    }
}

/// Dedicated label-management page: create, rename and delete the user's
/// opaque label names. Deletion is deliberately a two-stage interaction:
/// the server first reports the exact number of affected cards, then the
/// user must explicitly confirm the destructive second request.
#[allow(clippy::too_many_lines)]
#[component]
pub fn LabelManager() -> impl IntoView {
    let labels = RwSignal::new(None::<Vec<LabelResponse>>);
    let list_error = RwSignal::new(None::<String>);
    let action_error = RwSignal::new(None::<String>);
    let new_name = RwSignal::new(String::new());
    let editing = RwSignal::new(None::<i64>);
    let edit_value = RwSignal::new(String::new());
    let edit_error = RwSignal::new(None::<String>);
    let deleting = RwSignal::new(None::<(i64, String)>);
    let affected_cards = RwSignal::new(None::<i64>);
    let busy = RwSignal::new(false);

    let reload = Callback::new(move |(): ()| {
        leptos::task::spawn_local(async move {
            match crate::api::labels().await {
                Ok(found) => {
                    list_error.set(None);
                    labels.set(Some(found));
                }
                Err(err) => list_error.set(Some(err)),
            }
        });
    });

    #[cfg(feature = "csr")]
    Effect::new(move |_| reload.run(()));

    let create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let name = new_name.get_untracked().trim().to_owned();
        if name.is_empty() {
            action_error.set(Some("Enter a label name first.".to_owned()));
            return;
        }
        busy.set(true);
        action_error.set(None);
        leptos::task::spawn_local(async move {
            match crate::api::create_label(&name).await {
                Ok(_) => {
                    new_name.set(String::new());
                    busy.set(false);
                    reload.run(());
                }
                Err(err) => {
                    busy.set(false);
                    action_error.set(Some(err));
                }
            }
        });
    };

    let start_edit = move |(id, name): (i64, String)| {
        edit_error.set(None);
        edit_value.set(name);
        editing.set(Some(id));
    };

    let save_edit = move |(): ()| {
        let Some(id) = editing.get_untracked() else {
            return;
        };
        #[cfg(feature = "csr")]
        let old_name = labels
            .get_untracked()
            .and_then(|list| list.into_iter().find(|label| label.id == id))
            .map(|label| label.name);
        let name = edit_value.get_untracked().trim().to_owned();
        if name.is_empty() {
            edit_error.set(Some("Enter a label name first.".to_owned()));
            return;
        }
        busy.set(true);
        edit_error.set(None);
        leptos::task::spawn_local(async move {
            match crate::api::rename_label(id, &name).await {
                Ok(_) => {
                    #[cfg(feature = "csr")]
                    if let Some(old_name) = old_name.as_deref() {
                        preserve_selection_after_rename(old_name, id);
                    }
                    editing.set(None);
                    busy.set(false);
                    reload.run(());
                }
                Err(err) => {
                    busy.set(false);
                    edit_error.set(Some(err));
                }
            }
        });
    };

    let ask_delete = move |(id, name): (i64, String)| {
        action_error.set(None);
        affected_cards.set(None);
        deleting.set(Some((id, name)));
    };

    let delete = move |_| {
        let Some((id, _)) = deleting.get_untracked() else {
            return;
        };
        let confirm = affected_cards.get_untracked().is_some();
        busy.set(true);
        action_error.set(None);
        leptos::task::spawn_local(async move {
            match crate::api::delete_label(id, confirm).await {
                Ok(crate::api::DeleteLabelOutcome::Deleted) => {
                    deleting.set(None);
                    affected_cards.set(None);
                    busy.set(false);
                    reload.run(());
                }
                Ok(crate::api::DeleteLabelOutcome::NeedsConfirmation(count)) => {
                    affected_cards.set(Some(count));
                    busy.set(false);
                }
                Err(err) => {
                    busy.set(false);
                    action_error.set(Some(err));
                }
            }
        });
    };

    let cancel_delete = move |_| {
        deleting.set(None);
        affected_cards.set(None);
    };

    view! {
        <section class="labels-page" id="labels-page">
            <header class="page-header">
                <div>
                    <p class="page-kicker">"Collection"</p>
                    <h1>"Labels"</h1>
                    <p class="page-description">
                        "Create names to organize cards. Labels have no built-in meaning."
                    </p>
                </div>
            </header>
            <section class="labels-card" aria-labelledby="labels-heading">
                <div class="labels-card-heading">
                    <h2 id="labels-heading">"Your labels"</h2>
                    <span class="labels-count">
                        {move || labels.get().map_or_else(|| "".to_owned(), |list| list.len().to_string())}
                    </span>
                </div>
                <form id="create-label-form" class="label-create" on:submit=create>
                    <label for="new-label-name">"New label"</label>
                    <div class="label-create-row">
                        <input
                            type="text"
                            id="new-label-name"
                            maxlength="64"
                            autocomplete="off"
                            placeholder="e.g. Rust"
                            prop:value=move || new_name.get()
                            on:input=move |ev| {
                                new_name.set(event_target_value(&ev));
                                action_error.set(None);
                            }
                        />
                        <button type="submit" id="create-label" class="primary" disabled=move || busy.get()>
                            "Create"
                        </button>
                    </div>
                </form>
                {move || {
                    action_error
                        .get()
                        .map(|err| view! { <p class="error" id="labels-error" role="alert">{err}</p> })
                }}
                {move || {
                    if let Some(err) = list_error.get() {
                        view! { <p class="error" id="labels-list-error">{err}</p> }.into_any()
                    } else {
                        match labels.get() {
                            None => view! { <p class="labels-empty" id="labels-loading">"Loading labels…"</p> }.into_any(),
                            Some(list) if list.is_empty() => view! {
                                <p class="labels-empty" id="labels-empty">
                                    "No labels yet. Create one above or add a card with a new label."
                                </p>
                            }.into_any(),
                            Some(list) => view! {
                                <ul class="label-list" id="labels-list">
                                    {list.into_iter().map(|label| {
                                        let id = label.id;
                                        let name = label.name.clone();
                                        let row_id = format!("label-row-{id}");
                                        view! {
                                            <li class="label-row" id=row_id>
                                                {move || {
                                                    if editing.get() == Some(id) {
                                                        view! {
                                                            <div class="label-edit">
                                                                <input
                                                                    type="text"
                                                                    id="label-rename-input"
                                                                    maxlength="64"
                                                                    prop:value=move || edit_value.get()
                                                                    on:input=move |ev| {
                                                                        edit_value.set(event_target_value(&ev));
                                                                        edit_error.set(None);
                                                                    }
                                                                />
                                                                <button type="button" id="save-label-rename" on:click=move |_| save_edit(()) disabled=move || busy.get()>
                                                                    "Save"
                                                                </button>
                                                                <button type="button" id="cancel-label-rename" on:click=move |_| editing.set(None)>
                                                                    "Cancel"
                                                                </button>
                                                            </div>
                                                            {move || edit_error.get().map(|err| view! {
                                                                <p class="error label-row-error" id="label-rename-error">{err}</p>
                                                            })}
                                                        }.into_any()
                                                    } else {
                                                        let edit_name = name.clone();
                                                        let delete_name = name.clone();
                                                        view! {
                                                            <span class="label-row-info">
                                                                <span class="badge label">{name.clone()}</span>
                                                            </span>
                                                            <span class="label-row-actions">
                                                                <button type="button" id=format!("rename-label-{id}") on:click=move |_| start_edit((id, edit_name.clone()))>
                                                                    "Rename"
                                                                </button>
                                                                <button type="button" id=format!("delete-label-{id}") on:click=move |_| ask_delete((id, delete_name.clone()))>
                                                                    "Delete"
                                                                </button>
                                                            </span>
                                                        }.into_any()
                                                    }
                                                }}
                                            </li>
                                        }
                                    }).collect_view()}
                                </ul>
                            }.into_any(),
                        }
                    }
                }}
            </section>
            {move || deleting.get().map(|(_id, name)| {
                let warning = affected_cards.get();
                view! {
                    <div class="modal-backdrop" id="label-delete-modal">
                        <div class="modal" role="dialog" aria-modal="true" aria-labelledby="label-delete-title">
                            <h2 id="label-delete-title">"Delete label?"</h2>
                            <p class="modal-text">
                                "Delete “" {name} "”?"
                            </p>
                            {warning.map(|count| view! {
                                <p class="modal-progress-warning" id="label-delete-warning" role="alert">
                                    "This label is attached to " {count} " card" {if count == 1 { "" } else { "s" }} ". Deleting it will remove it from those cards."
                                </p>
                            })}
                            {move || action_error.get().map(|err| view! {
                                <p class="error" id="label-delete-error" role="alert">{err}</p>
                            })}
                            <div class="modal-buttons">
                                <button
                                    type="button"
                                    id="confirm-delete-label"
                                    class="failed"
                                    disabled=move || busy.get()
                                    on:click=delete
                                >
                                    {move || if affected_cards.get().is_some() { "Delete label and remove from cards" } else { "Check and delete" }}
                                </button>
                                <button type="button" id="cancel-delete-label" on:click=cancel_delete>
                                    "Cancel"
                                </button>
                            </div>
                        </div>
                    </div>
                }
            })}
        </section>
    }
}
