//! Groom tab: search, filter, page and maintain the whole card
//! collection.
//!
//! Search-as-you-type with a ~300 ms debounce (a generation counter makes
//! stale timers no-ops, so no cancel handle is needed); changing the query
//! resets to the first card and refetches. The clear button inside the box
//! empties the query immediately (no debounce — one click, one fetch)
//! and is disabled while the box is empty. Next to it, a filter
//! selects all (the first-usage default, owner decision 2026-07-31),
//! enabled or disabled cards (issue #127); changing it also resets to
//! the first card and refetches immediately (no debounce — one click, one fetch).
//! Both the filter and the search text persist in `localStorage`, so they
//! survive tab switches (a tab switch remounts this component) and
//! browser refresh alike; storage failures (private mode, disabled
//! storage) are ignored — persistence is a convenience, never fatal.
//! The page size fits the viewport (owner wish 2026-07-31): the first
//! fetch uses the persisted calibrated size (or a fallback), then ONE
//! calibration pass measures the real rendered rows and the free vertical
//! space and requests exactly the rows that fill the page — per-row
//! heights summed, so mixed one/two-line prompts still fit exactly on the
//! calibrated window (later windows can overflow slightly when their rows
//! are taller; the sticky chrome below keeps every control reachable
//! then). The list is offset-based, not page-number-based (owner feedback
//! 2026-07-31): the offset is the anchor, so a re-fit changes how many
//! cards show below the top one, never WHICH card is on top. Window
//! resizes re-fit (debounced, sub-row height-only changes ignored so
//! mobile URL-bar show/hide never churns the list). The server clamps
//! `take` and echoes the
//! effective size, which drives the skip arithmetic, the paging buttons
//! and the "showing X–Y of Z" line. A second generation counter guards
//! the fetch itself, so a stale in-flight response (rapid paging,
//! search-while-paging) can never overwrite newer rows. The search,
//! filter and paging chrome above the list is sticky below the (also
//! sticky) header. Row layout is two
//! lines: the clamped prompt, then a meta line with badges + due date on
//! the left and the actions right-aligned on the same line. Row actions:
//! edit (opens the card editor via the `on_edit` callback) and the
//! enable/disable toggle (immediate) stay inline; the rare destructive
//! ones (delete, reset-progress) sit in a per-row "⋯" menu behind a
//! confirm modal. Toggle and reset
//! refetch the current page; the toggle's refetch refreshes the badge in
//! place — or drops the row when the card no longer matches the active
//! filter (the sort — `next_time` asc, `id` tie-break — does not involve
//! `disabled`, so it never re-orders), while a progress
//! reset moves `next_time` and can genuinely re-order the page.
//! Deleting the last row of a page beyond the first steps back one page,
//! any other delete refetches in place so the count stays exact; a toggle
//! that drops the last row of a later page out of the active filter steps
//! back the same way.
//!
//! Loading, error (with retry) and empty states mirror the quiz tab.

use std::time::Duration;

use flasher_types::{CardResponse, CardState, DisabledFilter, MAX_TAKE};
use leptos::prelude::*;

use crate::api;

/// Page size of the very first fetch, before the viewport-fit calibration
/// has measured the real row heights (or when localStorage is
/// unavailable); matches the server default, so behavior without
/// calibration is unchanged.
const FALLBACK_PAGE_SIZE: usize = 10;

/// The server's `MAX_TAKE` clamp as the client's `usize` (mirrored so the
/// echoed `page_size` always equals the requested `take`).
const MAX_TAKE_USIZE: usize = MAX_TAKE as usize;

/// Search-as-you-type debounce delay.
const DEBOUNCE: Duration = Duration::from_millis(300);

/// Re-fit debounce on window resize (a drag fires a storm of events).
#[cfg(feature = "csr")]
const RESIZE_DEBOUNCE: Duration = Duration::from_millis(150);

/// `localStorage` keys under which the filter and the search text
/// persist (owner wish 2026-07-31: both survive tab switches — a tab
/// switch remounts this component — and browser refresh).
#[cfg(feature = "csr")]
const STORAGE_FILTER_KEY: &str = "flasher-groom-filter";
/// See [`STORAGE_FILTER_KEY`].
#[cfg(feature = "csr")]
const STORAGE_SEARCH_KEY: &str = "flasher-groom-search";
/// See [`STORAGE_FILTER_KEY`]; the calibrated viewport-fit page size, so
/// a remount or refresh starts with the right size instead of
/// overflowing and correcting.
#[cfg(feature = "csr")]
const STORAGE_TAKE_KEY: &str = "flasher-groom-take";

/// Reads a `localStorage` value; any storage failure (private mode,
/// disabled storage) yields `None` — persistence is a convenience, never
/// fatal.
#[cfg(feature = "csr")]
fn storage_get(key: &str) -> Option<String> {
    leptos::prelude::window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|storage| storage.get_item(key).ok())
        .flatten()
}

/// Writes a `localStorage` value, ignoring storage failures.
#[cfg(feature = "csr")]
fn storage_set(key: &str, value: &str) {
    if let Ok(Some(storage)) = leptos::prelude::window().local_storage() {
        let _ = storage.set_item(key, value);
    }
}

/// The persisted filter choice, or the first-usage default (`All`).
fn initial_filter() -> DisabledFilter {
    #[cfg(feature = "csr")]
    if let Some(filter) =
        storage_get(STORAGE_FILTER_KEY).and_then(|value| DisabledFilter::parse(&value))
    {
        return filter;
    }
    DisabledFilter::default()
}

/// The persisted search text (empty when none was stored).
fn initial_search() -> String {
    #[cfg(feature = "csr")]
    if let Some(search) = storage_get(STORAGE_SEARCH_KEY) {
        return search;
    }
    String::new()
}

/// The persisted viewport-fit page size (see [`STORAGE_TAKE_KEY`]), or
/// the fallback on first use. A stale value (the window changed since)
/// is corrected by the calibration pass.
fn initial_take() -> usize {
    #[cfg(feature = "csr")]
    if let Some(take) = storage_get(STORAGE_TAKE_KEY)
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|&take| take > 0)
    {
        return take.min(MAX_TAKE_USIZE);
    }
    FALLBACK_PAGE_SIZE
}

/// Rows that fit into `available_px`: the longest prefix of
/// `row_heights` (with `gap_px` between rows) whose summed height fits
/// — summing per-row because prompts clamp at two lines, so rows come
/// in two heights and a single average would under/overflow by a row
/// (adversarial review 2026-07-31). When every given row fits, the
/// remaining space is filled with rows of the TALLEST given height (the
/// only estimate available for unrendered rows; tallest never
/// overflows). Always at least one row (a broken measurement must never
/// produce an empty page), at most [`MAX_TAKE_USIZE`].
// Only the csr measurement glue and the unit tests call this.
#[cfg_attr(not(feature = "csr"), allow(dead_code))]
fn rows_that_fit(available_px: f64, row_heights: &[f64], gap_px: f64) -> usize {
    let mut used = 0.0;
    let mut count = 0;
    let mut tallest = 0.0_f64;
    for &height in row_heights {
        tallest = tallest.max(height);
        let next = used + height + gap_px;
        if next > available_px && count > 0 {
            return count;
        }
        used = next;
        count += 1;
    }
    // All rendered rows fit: fill the rest with tallest-seen rows (not
    // applicable when nothing was measured: tallest is 0 then).
    if tallest > 0.0 {
        let pitch = tallest + gap_px;
        while used + pitch <= available_px && count < MAX_TAKE_USIZE {
            used += pitch;
            count += 1;
        }
    }
    count.clamp(1, MAX_TAKE_USIZE)
}

/// What the viewport offers the groom list right now.
#[cfg(feature = "csr")]
struct ViewportFit {
    /// Free vertical space for the list in px, layout-referenced (the
    /// scroll position is folded in, so a scrolled page measures the
    /// same as an unscrolled one).
    available: f64,
    /// Height of every rendered row in px (without gap), in DOM order.
    row_heights: Vec<f64>,
    /// The list's row gap in px.
    row_gap: f64,
    /// The viewport width in px this measurement belongs to (the
    /// URL-bar churn guard keys on width changes).
    inner_width: f64,
}

/// Measures the free vertical space for `#groom-results` and the actual
/// rendered row heights. Everything is read live from the DOM/computed
/// styles — no CSS constants (owner wish 2026-07-31: no underfill from a
/// worst-case row assumption). The available space is what the list may
/// occupy without pushing the footer below the fold: viewport height
/// minus the list's layout top minus the footer height, the app gap and
/// the app bottom padding. `None` when no row is rendered yet (loading,
/// empty page) — the caller retries then.
#[cfg(feature = "csr")]
fn measure_viewport_fit() -> Option<ViewportFit> {
    use wasm_bindgen::JsCast;
    let window = leptos::prelude::window();
    let document = window.document()?;
    let results = document.get_element_by_id("groom-results")?;
    let app = document.query_selector(".app").ok()??;
    let footer = document.query_selector(".bottom").ok()??;
    let rows = document.query_selector_all(".groom-row").ok()?;
    let mut row_heights = Vec::with_capacity(rows.length() as usize);
    for index in 0..rows.length() {
        let row = rows.item(index)?;
        row_heights.push(f64::from(
            row.unchecked_into::<web_sys::HtmlElement>().offset_height(),
        ));
    }
    let row_gap = window
        .get_computed_style(&results)
        .ok()??
        .get_property_value("row-gap")
        .ok()
        .map_or(0.0, |value| px(&value));
    let app_style = window.get_computed_style(&app).ok()??;
    let app_gap = app_style
        .get_property_value("row-gap")
        .ok()
        .map_or(0.0, |value| px(&value));
    let app_padding_bottom = app_style
        .get_property_value("padding-bottom")
        .ok()
        .map_or(0.0, |value| px(&value));
    let footer_height = footer.get_bounding_client_rect().height();
    let inner_height = window.inner_height().ok()?.as_f64()?;
    let inner_width = window.inner_width().ok()?.as_f64()?;
    let scroll_y = window.scroll_y().ok()?;
    let results_top = results.get_bounding_client_rect().top() + scroll_y;
    Some(ViewportFit {
        available: inner_height - results_top - footer_height - app_gap - app_padding_bottom,
        row_heights,
        row_gap,
        inner_width,
    })
}

/// Parses a computed-style px value (`"16px"` → `16.0`); anything
/// unexpected degrades to 0 (the fit errs by a few px, never fatally).
#[cfg(feature = "csr")]
fn px(value: &str) -> f64 {
    value.trim_end_matches("px").trim().parse().unwrap_or(0.0)
}

/// Applies a measured fit: persists the measured take (also when
/// unchanged — the next mount, a tab switch remounts this component,
/// then starts right without a corrective refetch) and updates the take.
/// No re-anchoring: the list is offset-based, so the top card stays
/// exactly where it was (owner feedback 2026-07-31).
#[cfg(feature = "csr")]
fn apply_fit(fit_rows: usize, take: RwSignal<usize>) {
    storage_set(STORAGE_TAKE_KEY, &fit_rows.to_string());
    // No same-value set: any set notifies the fetch effect, and an
    // unchanged take must not cost a refetch.
    if fit_rows != take.get_untracked() {
        take.set(fit_rows);
    }
}

/// What the list area currently shows.
#[derive(Clone)]
enum LoadState {
    /// A fetch is in flight (also the initial state).
    Loading,
    /// One window of cards starting at the current offset, plus the
    /// total match count before paging (drives the "showing X–Y of Z"
    /// line).
    Loaded {
        cards: Vec<CardResponse>,
        count: i64,
    },
    /// A fetch or row action failed; retry refetches the current page.
    Error(String),
}

/// The destructive action an open confirm modal is armed with.
#[derive(Clone)]
enum ConfirmAction {
    /// Delete the card entirely.
    Delete(CardResponse),
    /// Reset the card's learning progress to state `new`.
    ResetProgress(CardResponse),
}

impl ConfirmAction {
    /// The card the action targets.
    fn card(&self) -> &CardResponse {
        match self {
            Self::Delete(card) | Self::ResetProgress(card) => card,
        }
    }

    /// The question shown above the prompt in the modal.
    fn question(&self) -> &'static str {
        match self {
            Self::Delete(_) => "Really delete this card?",
            Self::ResetProgress(_) => "Reset learning progress for this card?",
        }
    }

    /// The label of the confirm button.
    fn confirm_label(&self) -> &'static str {
        match self {
            Self::Delete(_) => "Delete",
            Self::ResetProgress(_) => "Reset",
        }
    }

    /// Shown in the modal when deleting a card that has learning progress
    /// (owner decision 2026-07-27: full delete stays available for learned
    /// cards, but the confirmation must surface the existing progress).
    fn progress_warning(&self) -> Option<String> {
        match self {
            Self::Delete(card) if card.state != CardState::New => {
                let state = match card.state {
                    CardState::Ok => "ok",
                    CardState::Failed => "failed",
                    CardState::New => unreachable!("guarded above"),
                };
                Some(format!(
                    "This card has learning progress (state: {state}). Deleting removes it permanently."
                ))
            }
            Self::Delete(_) | Self::ResetProgress(_) => None,
        }
    }
}

/// Converts a small UI count into the API's `i64` without a cast lint;
/// list arithmetic never leaves the low range.
fn as_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// The Groom tab.
// The list, paging and modal arms of the view make this long; splitting
// them further would only add indirection (same reasoning as the quiz).
#[allow(clippy::too_many_lines)]
#[component]
pub fn Groom(
    /// Opens the card editor for a row (owned by the App so the editor
    /// can overlay the tabs).
    on_edit: Callback<CardResponse>,
) -> impl IntoView {
    // The raw input value and the debounced query actually searched for;
    // both start from the persisted search text, so a tab switch (which
    // remounts this component) or a refresh resumes where the user left
    // off, and the first fetch already applies it.
    let input = RwSignal::new(initial_search());
    let query = RwSignal::new(input.get_untracked());
    // The all/enabled/disabled filter (issue #127): the persisted
    // choice, or `All` on first use (owner decision 2026-07-31).
    let filter = RwSignal::new(initial_filter());
    // The list is OFFSET-based, not page-number-based (owner feedback
    // 2026-07-31): the offset is the anchor, so a viewport re-fit changes
    // how many cards show below the top one — never WHICH card is on
    // top. Previous/Next step the offset by the take.
    let skip = RwSignal::new(0_usize);
    let state = RwSignal::new(LoadState::Loading);
    let confirm = RwSignal::new(None::<ConfirmAction>);
    // The requested page size (owner wish 2026-07-31: the list fills
    // exactly the viewport): the persisted calibrated value or the
    // fallback, corrected by the calibration pass and on resize.
    let take = RwSignal::new(initial_take());
    // Bumped by every fetch; a response that lands after a newer fetch was
    // armed is stale and must not touch the state (rapid paging).
    let fetch_generation = RwSignal::new(0_u64);

    // Fetches one window for a query; the single entry point for loading,
    // paging, retry and post-mutation refresh.
    let fetch = Callback::new(
        move |(q, f, s, t): (String, DisabledFilter, usize, usize)| {
            state.set(LoadState::Loading);
            fetch_generation.update(|count| *count += 1);
            let armed = fetch_generation.get_untracked();
            leptos::task::spawn_local(async move {
                let skip = u32::try_from(s).unwrap_or(u32::MAX);
                let take = u32::try_from(t).unwrap_or(u32::MAX);
                let result = api::find_cards(&q, f, skip, take).await;
                if fetch_generation.get_untracked() != armed {
                    // A newer fetch is already in flight; ignore this one.
                    return;
                }
                state.set(match result {
                    Ok(found) => LoadState::Loaded {
                        cards: found.cards,
                        count: found.count,
                    },
                    Err(err) => LoadState::Error(err),
                });
            });
        },
    );

    // (Re)fetch whenever the debounced query, the filter, the offset or
    // the requested page size changes.
    #[cfg(feature = "csr")]
    Effect::new(move |_| {
        fetch.run((query.get(), filter.get(), skip.get(), take.get()));
    });

    // Viewport-fit page size (owner wish 2026-07-31). The first fetch
    // uses the persisted take (or the fallback); the fit pass then
    // measures the real rendered rows and free vertical space and
    // corrects the take to exactly fill the viewport — per-row heights
    // summed, so mixed one/two-line prompts fit exactly (a worst-case
    // row assumption would underfill; owner feedback). A later page with
    // taller rows than measured can still overflow slightly — the sticky
    // header/controls keep everything reachable then, which is why this
    // needs no hard no-overflow guarantee.
    #[cfg(feature = "csr")]
    {
        // `needs_fit` arms the fit pass: on mount (initial calibration)
        // and whenever a resize measurement finds no rows (mid-refetch —
        // the pass then runs on the next Loaded render, so no re-fit is
        // ever lost in a Loading window; adversarial review 2026-07-31).
        let needs_fit = RwSignal::new(true);
        // (inner_width, available) of the last applied fit — the URL-bar
        // churn guard's baseline.
        let last_viewport = RwSignal::new(None::<(f64, f64)>);
        Effect::new(move |_| {
            if !needs_fit.get() {
                return;
            }
            let LoadState::Loaded { .. } = state.get() else {
                return;
            };
            // Defer the measurement past the render flush: this effect is
            // created before the view, so on `state.set(Loaded)` it runs
            // BEFORE the new rows are patched into the DOM — measuring
            // here would see the stale Loading state.
            set_timeout(
                move || {
                    if !needs_fit.get_untracked() {
                        return;
                    }
                    let Some(fit) = measure_viewport_fit() else {
                        // No row rendered yet (empty page): stay armed so
                        // a later non-empty render still fits.
                        return;
                    };
                    needs_fit.set(false);
                    last_viewport.set(Some((fit.inner_width, fit.available)));
                    apply_fit(
                        rows_that_fit(fit.available, &fit.row_heights, fit.row_gap),
                        take,
                    );
                },
                Duration::ZERO,
            );
        });

        // Re-fit on viewport changes, debounced like the search (a window
        // drag fires a storm of events). Mobile URL-bar show/hide also
        // fires resize with a sub-row height delta: a height-only change
        // smaller than one row is ignored, so plain scrolling never
        // churns the list (adversarial review 2026-07-31) — the fit can
        // then lag the viewport by less than a row, which the sticky
        // chrome absorbs.
        let resize_generation = RwSignal::new(0_u64);
        let resize = window_event_listener_untyped("resize", move |_| {
            resize_generation.update(|count| *count += 1);
            let armed = resize_generation.get_untracked();
            set_timeout(
                move || {
                    if resize_generation.get_untracked() != armed {
                        return;
                    }
                    let Some(fit) = measure_viewport_fit() else {
                        // Rows are mid-refetch: arm the fit pass, it runs
                        // on the next Loaded render.
                        needs_fit.set(true);
                        return;
                    };
                    let tallest = fit.row_heights.iter().copied().fold(0.0_f64, f64::max);
                    let sub_row_noise = matches!(
                        last_viewport.get_untracked(),
                        Some((width, available))
                            if (width - fit.inner_width).abs() < 1.0
                                && (available - fit.available).abs() < tallest + fit.row_gap
                    );
                    if sub_row_noise {
                        return;
                    }
                    last_viewport.set(Some((fit.inner_width, fit.available)));
                    apply_fit(
                        rows_that_fit(fit.available, &fit.row_heights, fit.row_gap),
                        take,
                    );
                },
                RESIZE_DEBOUNCE,
            );
        });
        let _keep_resize = StoredValue::new(resize);
    }

    // Search-as-you-type: every keystroke bumps the generation and arms a
    // timer; when it fires and is still the latest, the query goes live.
    // `batch` makes the offset reset and the query change a single trigger,
    // so the effect fires one fetch, not two.
    let generation = RwSignal::new(0_u64);
    let on_search_input = move |ev: leptos::ev::Event| {
        let value = event_target_value(&ev);
        input.set(value.clone());
        generation.update(|count| *count += 1);
        let armed = generation.get_untracked();
        set_timeout(
            move || {
                if generation.get_untracked() == armed {
                    // Persist the LIVE query (not every keystroke), so a
                    // refresh resumes with exactly what the list showed.
                    #[cfg(feature = "csr")]
                    storage_set(STORAGE_SEARCH_KEY, &value);
                    batch(|| {
                        skip.set(0);
                        query.set(value);
                    });
                }
            },
            DEBOUNCE,
        );
    };

    // Clear button: empties the box immediately — one click, one fetch,
    // like the filter, no debounce. Bumping the generation first makes
    // any pending debounce timer a no-op, so a keystroke armed just
    // before the click cannot resurrect the old query afterwards.
    let on_clear_search = move |_| {
        generation.update(|count| *count += 1);
        input.set(String::new());
        #[cfg(feature = "csr")]
        storage_set(STORAGE_SEARCH_KEY, "");
        batch(|| {
            skip.set(0);
            query.set(String::new());
        });
    };

    // Filter change: like a search change — reset to the first card and
    // refetch (the `batch` makes both one effect trigger), but immediate:
    // a select needs no debounce. Unknown values are impossible from the
    // rendered options and are ignored. The choice persists across tab
    // switches and refresh (localStorage).
    let on_filter_change = move |ev: leptos::ev::Event| {
        if let Some(value) = DisabledFilter::parse(&event_target_value(&ev)) {
            #[cfg(feature = "csr")]
            storage_set(STORAGE_FILTER_KEY, value.as_str());
            batch(|| {
                skip.set(0);
                filter.set(value);
            });
        }
    };

    // Enable/disable toggle — immediate, no confirmation. `disabled` is
    // not part of the server-side ordering, so the row cannot jump; the
    // refetch refreshes the badge/label in place — or drops the row when
    // it no longer matches the active filter (e.g. disabling a card
    // under the `enabled` filter), which is exactly what the
    // filter promises. A row that drops out as the last one of a later
    // page steps back one page (same fallback as delete), so the user
    // never lands on a false "No cards match." without a way back.
    let toggle_disabled = Callback::new(move |card: CardResponse| {
        leptos::task::spawn_local(async move {
            match api::set_disabled(&card.id, !card.disabled).await {
                Ok(_updated) => {
                    // `card` is the pre-toggle render: the row leaves the
                    // list when the NEW state misses the filter.
                    let drops_out = match filter.get_untracked() {
                        DisabledFilter::Enabled => !card.disabled,
                        DisabledFilter::Disabled => card.disabled,
                        DisabledFilter::All => false,
                    };
                    let was_single = matches!(
                        state.get_untracked(),
                        LoadState::Loaded { ref cards, .. } if cards.len() == 1
                    );
                    if drops_out && was_single && skip.get_untracked() > 0 {
                        skip.update(|s| *s = s.saturating_sub(take.get_untracked()));
                    } else {
                        fetch.run((
                            query.get_untracked(),
                            filter.get_untracked(),
                            skip.get_untracked(),
                            take.get_untracked(),
                        ));
                    }
                }
                Err(err) => state.set(LoadState::Error(err)),
            }
        });
    });

    // Confirmed delete: step back one page when the last row of a later
    // page vanished (the effect refetches), otherwise refetch in place so
    // the next card slides in and the count stays exact.
    let do_delete = Callback::new(move |card: CardResponse| {
        leptos::task::spawn_local(async move {
            match api::delete_card(&card.id).await {
                Ok(()) => {
                    let was_single = matches!(
                        state.get_untracked(),
                        LoadState::Loaded { ref cards, .. } if cards.len() == 1
                    );
                    if was_single && skip.get_untracked() > 0 {
                        skip.update(|s| *s = s.saturating_sub(take.get_untracked()));
                    } else {
                        fetch.run((
                            query.get_untracked(),
                            filter.get_untracked(),
                            skip.get_untracked(),
                            take.get_untracked(),
                        ));
                    }
                }
                Err(err) => state.set(LoadState::Error(err)),
            }
        });
    });

    // Confirmed progress reset: the reset can change the ordering
    // (next_time moves), so the current page is refetched.
    let do_reset = Callback::new(move |card: CardResponse| {
        leptos::task::spawn_local(async move {
            match api::delete_history(&card.id).await {
                Ok(_updated) => fetch.run((
                    query.get_untracked(),
                    filter.get_untracked(),
                    skip.get_untracked(),
                    take.get_untracked(),
                )),
                Err(err) => state.set(LoadState::Error(err)),
            }
        });
    });

    view! {
        <section class="groom">
            // Sticky chrome (owner wish 2026-07-31): search/filter and
            // the paging bar stay directly below the sticky header while
            // the list scrolls (CSS: .groom-head, top: var(--top-h)).
            <div class="groom-head">
                // No visible "Search" label (owner decision 2026-07-31): the
                // placeholder makes the box self-explanatory and the label
                // only cost a line; the aria-label keeps the accessible name.
                <div class="groom-controls">
                    // The × sits INSIDE the search box (owner wish
                    // 2026-07-31): it clears the box, so it should look like
                    // part of it, not like a third control next to it.
                    <div class="groom-search">
                        <input
                            id="groom-search"
                            type="text"
                            placeholder="Search cards…"
                            aria-label="Search cards"
                            prop:value=input
                            on:input=on_search_input
                        />
                        <button
                            type="button"
                            id="groom-clear"
                            aria-label="Clear search"
                            title="Clear search"
                            disabled=move || input.get().is_empty()
                            on:click=on_clear_search
                        >
                            "×"
                        </button>
                    </div>
                    <select
                        id="groom-filter"
                        aria-label="Filter cards by status"
                        prop:value=move || filter.get().as_str()
                        on:change=on_filter_change
                    >
                        <option value="enabled">"Enabled"</option>
                        <option value="disabled">"Disabled"</option>
                        <option value="all">"All"</option>
                    </select>
                </div>
                {move || {
                    let LoadState::Loaded { cards, count } = state.get() else {
                        return None;
                    };
                    if cards.is_empty() {
                        return None;
                    }
                    // Paging bar ABOVE the list (Phase 6.5, owner
                    // complaint): on a full page the user no longer
                    // has to scroll to reach Previous/Next. The hit
                    // count sits left; the buttons sit as a close
                    // pair at the right edge, flush with the card
                    // rows (owner wish 2026-07-31: less mouse
                    // travel between Previous and Next). The buttons
                    // step the offset by the take.
                    let offset = skip.get();
                    let first = as_i64(offset) + 1;
                    let last = as_i64(offset + cards.len());
                    let has_prev = offset > 0;
                    let has_next = last < count;
                    Some(view! {
                        <div class="groom-paging">
                            <span id="groom-page-info">
                                {format!("showing {first}–{last} of {count}")}
                            </span>
                            <button
                                type="button"
                                id="groom-prev"
                                disabled=!has_prev
                                on:click=move |_| {
                                    skip.update(|s| *s = s.saturating_sub(take.get_untracked()));
                                }
                            >
                                "Previous"
                            </button>
                            <button
                                type="button"
                                id="groom-next"
                                disabled=!has_next
                                on:click=move |_| {
                                    skip.update(|s| *s += take.get_untracked());
                                }
                            >
                                "Next"
                            </button>
                        </div>
                    })
                }}
            </div>
            {move || match state.get() {
                LoadState::Loading => view! {
                    <p class="groom-status" id="groom-loading">"Loading cards…"</p>
                }
                    .into_any(),
                LoadState::Error(err) => view! {
                    <div class="groom-status" id="groom-error">
                        <p class="form-error">"Something went wrong: " {err}</p>
                        <button
                            type="button"
                            id="groom-retry"
                            on:click=move |_| {
                                fetch.run((
                                    query.get_untracked(),
                                    filter.get_untracked(),
                                    skip.get_untracked(),
                                    take.get_untracked(),
                                ));
                            }
                        >
                            "Retry"
                        </button>
                    </div>
                }
                    .into_any(),
                LoadState::Loaded { cards, .. } => {
                    if cards.is_empty() {
                        view! {
                            <p class="groom-status" id="groom-empty">"No cards match."</p>
                        }
                        .into_any()
                    } else {
                        view! {
                            <div class="groom-results" id="groom-results">
                                {cards
                                    .into_iter()
                                    .map(|card| {
                                        view! {
                                            <GroomRow
                                                card=card
                                                on_edit=on_edit
                                                toggle_disabled=toggle_disabled
                                                ask_delete=Callback::new(move |card| {
                                                    confirm.set(Some(ConfirmAction::Delete(card)));
                                                })
                                                ask_reset=Callback::new(move |card| {
                                                    confirm.set(Some(
                                                        ConfirmAction::ResetProgress(card),
                                                    ));
                                                })
                                            />
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                        .into_any()
                    }
                }
            }}
            {move || confirm.get().map(|action| {
                let card = action.card().clone();
                let question = action.question();
                let confirm_label = action.confirm_label();
                let progress_warning = action.progress_warning();
                // Delete is destructive (red); reset progress is not.
                let destructive = matches!(action, ConfirmAction::Delete(_));
                view! {
                    <div class="modal-backdrop" id="groom-modal">
                        <div class="modal" role="dialog" aria-modal="true">
                            <p class="modal-text" id="groom-modal-text">
                                {question}
                                <br />
                                <span class="modal-prompt">{card.prompt}</span>
                            </p>
                            {progress_warning.map(|warning| view! {
                                <p class="modal-progress-warning" id="modal-progress-warning">
                                    {warning}
                                </p>
                            })}
                            <div class="modal-buttons">
                                <button
                                    type="button"
                                    id="modal-confirm"
                                    class:failed=destructive
                                    on:click=move |_| {
                                        confirm.set(None);
                                        match &action {
                                            ConfirmAction::Delete(card) => {
                                                do_delete.run(card.clone());
                                            }
                                            ConfirmAction::ResetProgress(card) => {
                                                do_reset.run(card.clone());
                                            }
                                        }
                                    }
                                >
                                    {confirm_label}
                                </button>
                                <button
                                    type="button"
                                    id="modal-cancel"
                                    on:click=move |_| confirm.set(None)
                                >
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

/// One card row of the groom list: truncated prompt on the first line,
/// and a single meta line below it — state badge (plus a `disabled`
/// badge) and due date on the left, the row actions right-aligned on
/// the SAME line (owner decision: one visual row less per card than the
/// old three-line layout). The everyday, reversible actions (edit,
/// enable/disable) stay one click away; the rare destructive ones
/// (reset progress, delete) live in a "⋯" overflow menu and still arm
/// the same confirm modal. The menu closes via a transparent
/// full-viewport backdrop — the same pattern the modal uses, so no
/// window listeners are needed.
#[component]
fn GroomRow(
    card: CardResponse,
    on_edit: Callback<CardResponse>,
    toggle_disabled: Callback<CardResponse>,
    ask_delete: Callback<CardResponse>,
    ask_reset: Callback<CardResponse>,
) -> impl IntoView {
    let id = card.id.clone();
    let row_id = format!("groom-row-{id}");
    let state_badge_id = format!("state-{id}");
    let disabled_badge_id = format!("disabled-{id}");
    let due_id = format!("due-{id}");
    let edit_id = format!("edit-{id}");
    let toggle_id = format!("toggle-disabled-{id}");
    let menu_id = format!("menu-{id}");
    let reset_id = format!("reset-{id}");
    let delete_id = format!("delete-{id}");
    let state = card.state.as_str();
    let disabled = card.disabled;
    let due = due_label(card.next_time);
    let toggle_label = if disabled { "Enable" } else { "Disable" };
    let menu_open = RwSignal::new(false);
    // One owned clone per click handler (each handler moves its capture).
    let card_edit = card.clone();
    let card_toggle = card.clone();
    let card_reset = card.clone();
    let card_delete = card.clone();

    view! {
        <div class="groom-row" id=row_id>
            <p class="groom-prompt">{card.prompt.clone()}</p>
            <div class="groom-meta">
                <span class=format!("badge state-{state}") id=state_badge_id>
                    {state}
                </span>
                {disabled.then(|| {
                    view! {
                        <span class="badge disabled" id=disabled_badge_id>
                            "disabled"
                        </span>
                    }
                })}
                <span class="groom-due" id=due_id>{due}</span>
                <div class="groom-actions">
                    <button
                        type="button"
                        id=edit_id
                        on:click=move |_| on_edit.run(card_edit.clone())
                    >
                        "Edit"
                    </button>
                    <button
                        type="button"
                        id=toggle_id
                        on:click=move |_| toggle_disabled.run(card_toggle.clone())
                    >
                        {toggle_label}
                    </button>
                    <button
                        type="button"
                        class="groom-menu-button"
                        id=menu_id
                        aria-label="More actions"
                        aria-expanded=move || menu_open.get().to_string()
                        on:click=move |_| menu_open.update(|open| *open = !*open)
                    >
                        "⋯"
                    </button>
                    {move || {
                        menu_open.get().then(|| {
                            let reset_id = reset_id.clone();
                            let delete_id = delete_id.clone();
                            let card_reset = card_reset.clone();
                            let card_delete = card_delete.clone();
                            view! {
                                <div
                                    class="groom-menu-backdrop"
                                    on:click=move |_| menu_open.set(false)
                                ></div>
                                // Deliberately a plain group of buttons,
                                // not role="menu": the app has no
                                // keyboard menu semantics anywhere (no
                                // Escape/arrow handling — the modal is
                                // the same), and the role would promise
                                // them (adversarial review 2026-07-31).
                                <div class="groom-menu">
                                    <button
                                        type="button"
                                        id=reset_id
                                        on:click=move |_| {
                                            menu_open.set(false);
                                            ask_reset.run(card_reset.clone());
                                        }
                                    >
                                        "Reset progress"
                                    </button>
                                    <button
                                        type="button"
                                        class="failed"
                                        id=delete_id
                                        on:click=move |_| {
                                            menu_open.set(false);
                                            ask_delete.run(card_delete.clone());
                                        }
                                    >
                                        "Delete"
                                    </button>
                                </div>
                            }
                        })
                    }}
                </div>
            </div>
        </div>
    }
}

/// Human-friendly due label: `due now` when the time has passed,
/// otherwise `due YYYY-MM-DD` (UTC, formatted by `flasher-core`).
fn due_label(next_time: i64) -> String {
    if next_time <= now_ms() {
        "due now".to_owned()
    } else {
        format!("due {}", flasher_core::format_utc_date(next_time))
    }
}

/// Current unix epoch millis via the browser clock (wasm-only).
#[cfg(feature = "csr")]
fn now_ms() -> i64 {
    // Date::now() is milliseconds since the epoch as f64; the value is
    // far from any truncation boundary that matters here.
    #[allow(clippy::cast_possible_truncation)]
    {
        js_sys::Date::now() as i64
    }
}

/// SSR stand-in: the initial groom render is the Loading state, so no due
/// label is ever computed server-side; this only keeps the view linkable.
#[cfg(not(feature = "csr"))]
fn now_ms() -> i64 {
    0
}

#[cfg(test)]
mod tests {
    use super::rows_that_fit;

    #[test]
    fn rows_that_fit_sums_the_prefix() {
        // Uniform rows: classic floor division.
        assert_eq!(rows_that_fit(250.0, &[100.0, 100.0, 100.0], 0.0), 2);
        // Mixed heights, one px short of three rows: the prefix stops at
        // two no matter where the tall row sits; at exactly 250 all
        // three fit.
        assert_eq!(rows_that_fit(249.0, &[100.0, 100.0, 50.0], 0.0), 2);
        assert_eq!(rows_that_fit(249.0, &[100.0, 50.0, 100.0], 0.0), 2);
        assert_eq!(rows_that_fit(249.0, &[50.0, 100.0, 100.0], 0.0), 2);
        assert_eq!(rows_that_fit(250.0, &[100.0, 100.0, 50.0], 0.0), 3);
        // The gap counts between rows.
        assert_eq!(rows_that_fit(205.0, &[100.0, 100.0], 10.0), 1);
        assert_eq!(rows_that_fit(220.0, &[100.0, 100.0], 10.0), 2);
    }

    #[test]
    fn rows_that_fit_never_returns_zero() {
        // No rows measured, no space, or a first row taller than the
        // viewport: still one row, never an empty page.
        assert_eq!(rows_that_fit(500.0, &[], 10.0), 1);
        assert_eq!(rows_that_fit(0.0, &[100.0], 10.0), 1);
        assert_eq!(rows_that_fit(-50.0, &[100.0], 10.0), 1);
        assert_eq!(rows_that_fit(50.0, &[100.0, 100.0], 0.0), 1);
    }

    #[test]
    fn rows_that_fit_fills_with_the_tallest_seen_when_all_fit() {
        // All rendered rows fit: the rest is filled with tallest-seen
        // rows (the only estimate for unrendered ones).
        assert_eq!(rows_that_fit(400.0, &[80.0, 80.0], 20.0), 4);
        // The server's clamp is mirrored, so the echoed page size always
        // equals the requested take.
        assert_eq!(rows_that_fit(1_000_000.0, &[1.0], 0.0), 100);
    }
}

/// `px` only exists under csr (its caller is the DOM measurement glue),
/// so its tests only compile there.
#[cfg(all(test, feature = "csr"))]
mod csr_tests {
    use super::px;

    /// f64 equality without the `float_cmp` lint.
    fn approx(actual: f64, expected: f64) -> bool {
        (actual - expected).abs() < 1e-9
    }

    #[test]
    fn px_parses_computed_style_values() {
        assert!(approx(px("16px"), 16.0));
        assert!(approx(px("8.5px"), 8.5));
        // Anything unexpected degrades to 0 (a few px of measurement
        // error, never a failure).
        assert!(approx(px(""), 0.0));
        assert!(approx(px("auto"), 0.0));
        assert!(approx(px("0.75rem"), 0.0));
    }
}
