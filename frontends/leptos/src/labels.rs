//! Shared label filter for the groom and quiz tabs (owner decision
//! 2026-08-01): a quiet "Labels (n/m) ▾" button opening a checkbox
//! panel — union semantics (a card matches when it carries ANY selected
//! label). The selection is owned by the parent and persisted per page
//! in `localStorage`, so the groom and quiz selections are independent.

use flasher_types::LabelResponse;
use leptos::prelude::*;

/// Joins a label selection for the wire and `localStorage`. (The seed
/// names contain no comma; creation with arbitrary names is future work
/// and will keep the constraint.)
pub fn join_labels(selected: &[String]) -> String {
    selected.join(",")
}

/// Splits a persisted/wire selection back into names. (Only csr code —
/// the persisted-selection initialization — calls this.)
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
pub fn split_labels(raw: &str) -> Vec<String> {
    raw.split(',')
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
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
    use super::toggle_label_name;

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
    /// The selected names (owned by the parent; updated via `on_change`).
    selected: RwSignal<Vec<String>>,
    /// Called with the new selection on every checkbox change.
    on_change: Callback<Vec<String>>,
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
                    .map(|name| {
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
                                    let name = label.name.clone();
                                    let box_id = format!("{id_prefix}-label-{name}");
                                    let on_toggle = {
                                        let name = name.clone();
                                        move |ev: leptos::ev::Event| {
                                            let next = toggle_label_name(
                                                &selected.get_untracked(),
                                                &name,
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
                                                prop:checked=move || selected.get().contains(&name)
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
