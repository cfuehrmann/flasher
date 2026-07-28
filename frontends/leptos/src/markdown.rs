//! Rendered Markdown + `KaTeX` for card content (Phase 4A).
//!
//! Pipeline: `pulldown-cmark` (GFM tables + `$`/`$$` math, matching what
//! the real card collection uses) emits HTML, `ammonia` sanitizes it with
//! its default allow-list (tables survive; raw HTML in card content —
//! scripts, event handlers — is stripped), and the result is injected via
//! `inner_html`. Math is then typeset client-side by `KaTeX`'s auto-render
//! extension (vendored under `vendor/katex`, served at `/katex/`).
//!
//! Math handling: `pulldown-cmark`'s `ENABLE_MATH` isolates `$...$` /
//! `$$...$$` spans from Markdown processing (so `_`, `*`, `\` inside
//! formulas are never mangled by emphasis/escape rules). The math events
//! are re-emitted as *text* wrapped in `\(...\)` / `\[...\]` — the
//! delimiters `KaTeX` auto-render is configured with — so the HTML emitter
//! escapes `<`/`&` inside formulas correctly.
//!
//! Bare URLs: the old app (remark-gfm) autolinked them, `pulldown-cmark`
//! 0.13 has no such option — and the real card collection contains bare
//! URLs. A pre-parse pass ([`autolink_bare_urls`]) therefore wraps bare
//! `http(s)` URLs in `CommonMark` autolinks (`<https://…>`) before parsing,
//! leaving URLs inside `[text](url)` destinations, existing `<url>`
//! autolinks, code spans and code fences untouched. GFM's strikethrough
//! and tasklists are deliberately NOT enabled: the real collection has
//! zero occurrences of either.

use leptos::prelude::*;
use pulldown_cmark::{Event, Options, Parser};

/// Render Markdown to sanitized HTML, ready for `inner_html`.
fn render_html(markdown: &str) -> String {
    let autolinked = autolink_bare_urls(markdown);
    let parser = Parser::new_ext(&autolinked, Options::ENABLE_TABLES | Options::ENABLE_MATH);
    // Re-emit math as escaped text in KaTeX auto-render delimiters (see
    // module docs). `Event::Text` goes through the emitter's HTML
    // escaping, so `<`/`&` in formulas cannot break out into markup.
    let events = parser.map(|event| match event {
        Event::InlineMath(math) => Event::Text(format!("\\({math}\\)").into()),
        Event::DisplayMath(math) => Event::Text(format!("\\[{math}\\]").into()),
        other => other,
    });
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, events);
    ammonia::clean(&html)
}

/// Wrap bare `http://`/`https://` URLs in `CommonMark` autolinks (`<url>`),
/// so `pulldown-cmark` links them like the old app's remark-gfm did.
///
/// Left untouched (see module docs): URLs inside `[text](url)` link
/// destinations, inside existing `<url>` autolinks, and inside inline
/// code spans or fenced code blocks. Code spans are tracked per line —
/// an opening backtick run without its match swallows the rest of the
/// line, and spans crossing lines are not tracked; both are fine for
/// card content.
///
/// Trailing punctuation follows the common autolink-literal behavior:
/// `.,;:!?` never belong to the URL, and a trailing `)` is stripped only
/// while it is unbalanced (so Wikipedia-style `…/A_(b)` keeps its parens
/// while `(see https://x/y)` drops the closing one).
fn autolink_bare_urls(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len() + 16);
    // State of a fenced code block: (marker char, length of opening run).
    let mut fenced = None::<(u8, usize)>;
    for line in markdown.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        if let Some((marker, len)) = fenced {
            out.push_str(line);
            if fence_run(body).is_some_and(|(m, l, rest)| {
                m == marker && l >= len && rest.trim_start_matches(' ').is_empty()
            }) {
                fenced = None;
            }
            continue;
        }
        if let Some((marker, len, _)) = fence_run(body) {
            fenced = Some((marker, len));
            out.push_str(line);
            continue;
        }
        autolink_line(body, &mut out);
        if line.len() != body.len() {
            out.push('\n');
        }
    }
    out
}

/// Leading fence run of a line: up to three spaces, then a run of at
/// least three backticks or tildes. Returns `(marker, run length, rest)`.
fn fence_run(body: &str) -> Option<(u8, usize, &str)> {
    let s = body.trim_start_matches(' ');
    if body.len() - s.len() > 3 {
        return None;
    }
    let &marker = s.as_bytes().first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let len = s.bytes().take_while(|&b| b == marker).count();
    (len >= 3).then(|| (marker, len, &s[len..]))
}

/// One non-fence line of [`autolink_bare_urls`]: links bare URLs outside
/// inline code spans, existing autolinks and link destinations.
fn autolink_line(line: &str, out: &mut String) {
    let bytes = line.as_bytes();
    let mut i = 0;
    // Length of the backtick run that opened the code span we are in.
    let mut code_span = None::<usize>;
    while i < line.len() {
        let rest = &line[i..];
        if rest.starts_with('`') {
            let run = rest.bytes().take_while(|&b| b == b'`').count();
            // A run matching the opening run closes the span (CommonMark);
            // a different-length run inside a span is literal text.
            if code_span == Some(run) {
                code_span = None;
            } else if code_span.is_none() {
                code_span = Some(run);
            }
            out.push_str(&rest[..run]);
            i += run;
            continue;
        }
        if code_span.is_none()
            && (rest.starts_with("http://") || rest.starts_with("https://"))
            && !is_link_destination(line, i)
            // Already inside a `<url>` autolink.
            && bytes[..i].last() != Some(&b'<')
        {
            let mut end = i;
            while end < line.len() && !matches!(bytes[end], b' ' | b'\t' | b'`' | b'<') {
                end += 1;
            }
            let end = trim_trailing_punctuation(&line[i..end]) + i;
            out.push('<');
            out.push_str(&line[i..end]);
            out.push('>');
            i = end;
            continue;
        }
        let len = rest.chars().next().map_or(1, char::len_utf8);
        out.push_str(&rest[..len]);
        i += len;
    }
}

/// True when the URL starting at byte `i` is the destination of a
/// `[text](url)` link: a `(` (optionally followed by spaces) preceded by
/// a `]`. A bare parenthesized URL like `(see https://x)` is NOT a link
/// destination and still gets autolinked.
fn is_link_destination(line: &str, i: usize) -> bool {
    let before = line[..i].trim_end_matches([' ', '\t']).as_bytes();
    before.last() == Some(&b'(') && before.len() >= 2 && before[before.len() - 2] == b']'
}

/// Length of the URL prefix of `url` without its trailing punctuation
/// (see [`autolink_bare_urls`] for the rules).
fn trim_trailing_punctuation(url: &str) -> usize {
    let bytes = url.as_bytes();
    let mut end = url.len();
    // opens − closes over the current prefix; updated as `)` are stripped.
    let mut paren_balance: i64 = url
        .bytes()
        .map(|b| i64::from(b == b'(') - i64::from(b == b')'))
        .sum();
    loop {
        match bytes[..end].last() {
            Some(b'.' | b',' | b';' | b':' | b'!' | b'?') => end -= 1,
            // Strip a trailing `)` only while unbalanced.
            Some(b')') if paren_balance < 0 => {
                end -= 1;
                paren_balance += 1;
            }
            _ => break,
        }
    }
    end
}

/// Card content rendered as Markdown with `KaTeX` typesetting.
///
/// Used for both the quiz prompt and the revealed solution. `KaTeX`
/// re-runs whenever `markdown` changes (quiz navigation, solution
/// reveal).
#[component]
pub fn MarkdownView(
    /// Raw Markdown card content.
    #[prop(into)]
    markdown: Signal<String>,
    /// Element id; the `KaTeX` pass locates the node by it.
    id: &'static str,
    /// Extra CSS class (e.g. `"solution"`).
    #[prop(optional)]
    class: &'static str,
) -> impl IntoView {
    let html = Memo::new(move |_| render_html(&markdown.get()));

    // Re-typeset math whenever the rendered content changes — but only
    // when math is actually present: `schedule_render` lazily injects the
    // vendored KaTeX assets, and pages without math (e.g. the empty
    // editor preview) should not pay that download/parse cost.
    #[cfg(feature = "csr")]
    Effect::new(move |_| {
        if needs_katex(&html.get()) {
            katex::schedule_render(id);
        }
    });

    view! {
        // SAFETY (the one sanctioned inner_html in the app): `html` is
        // not raw user input — it is the output of the pulldown-cmark →
        // ammonia pipeline above. ammonia's default allow-list strips
        // every tag/attribute that can execute script (`<script>`,
        // `on*=` handlers, `javascript:` URLs, ...), so injecting the
        // sanitized result here cannot introduce XSS.
        <div id=id class=format!("md {class}") inner_html=move || html.get()></div>
    }
}

/// `KaTeX` auto-render glue (browser only; under ssr no math is typeset
/// and the `\(...\)` delimiters would just sit there unused anyway).
///
/// The vendored `KaTeX` assets (`/katex/...`, copied into `dist/` by
/// Trunk) are NOT in `index.html`: they are injected lazily on the first
/// card render, so the initial page load carries neither the
/// render-blocking CSS nor the script parsing (Lighthouse LCP/TTI).
/// `needs_katex` additionally suppresses the injection entirely for
/// content without math (the pipeline emits math as `\(`/`\[` text).
#[cfg(any(feature = "csr", test))]
fn needs_katex(rendered_html: &str) -> bool {
    rendered_html.contains("\\(") || rendered_html.contains("\\[")
}
#[cfg(feature = "csr")]
mod katex {
    use std::time::Duration;

    use js_sys::{Function, JSON, Reflect};
    use leptos::prelude::{document, request_animation_frame, set_timeout, window};
    use wasm_bindgen::JsValue;

    /// auto-render configuration: only `\(...\)` / `\[...\]`, matching
    /// what `render_html` emits; never throw on a broken formula.
    const OPTIONS_JSON: &str = r#"{
        "delimiters": [
            {"left": "\\[", "right": "\\]", "display": true},
            {"left": "\\(", "right": "\\)", "display": false}
        ],
        "throwOnError": false
    }"#;

    /// The injected scripts need a moment to arrive and execute, and the
    /// first card can render before `renderMathInElement` exists. Poll
    /// briefly — ~2 s at 50 ms intervals — before giving up; the raw
    /// `\(...\)` text stays readable in that unlikely case.
    const MAX_ATTEMPTS: u32 = 40;
    /// Interval between availability probes.
    const RETRY_INTERVAL: Duration = Duration::from_millis(50);

    /// Typeset the math inside the `#id` element once the DOM has
    /// settled (hence the animation frame: the `inner_html` update is
    /// itself effect-driven and must land first).
    pub fn schedule_render(id: &'static str) {
        ensure_assets_loaded();
        request_animation_frame(move || attempt(id, 0));
    }

    /// Inject the `KaTeX` stylesheet and scripts into `<head>` once.
    /// `async = false` on the scripts keeps their execution order:
    /// `katex.min.js` defines the `katex` global that `auto-render.min.js`
    /// uses at call time.
    fn ensure_assets_loaded() {
        let document = document();
        let Some(head) = document.head() else {
            return;
        };
        if document.get_element_by_id("katex-css").is_none()
            && let Ok(link) = document.create_element("link")
        {
            link.set_id("katex-css");
            _ = link.set_attribute("rel", "stylesheet");
            _ = link.set_attribute("href", "/katex/katex.min.css");
            _ = head.append_child(&link);
        }
        for (id, src) in [
            ("katex-js", "/katex/katex.min.js"),
            ("katex-auto-render", "/katex/auto-render.min.js"),
        ] {
            if document.get_element_by_id(id).is_some() {
                continue;
            }
            if let Ok(script) = document.create_element("script") {
                script.set_id(id);
                _ = script.set_attribute("src", src);
                // Not an HTML attribute: dynamically inserted scripts
                // default to async, which would break the order.
                _ = Reflect::set(&script, &JsValue::from_str("async"), &JsValue::FALSE);
                _ = head.append_child(&script);
            }
        }
    }

    fn attempt(id: &'static str, tries: u32) {
        if try_render(id) || tries >= MAX_ATTEMPTS {
            return;
        }
        set_timeout(move || attempt(id, tries + 1), RETRY_INTERVAL);
    }

    /// One render attempt. Returns `true` when done (rendered, or the
    /// element is gone, or the options are unusable) and `false` when
    /// the assets are not loaded yet and a retry is worthwhile.
    fn try_render(id: &str) -> bool {
        let window = JsValue::from(window());
        let global = |name: &str| Reflect::get(&window, &JsValue::from_str(name));
        // Both globals are needed: `renderMathInElement` calls `katex`
        // internally, and the two scripts load independently.
        let (Ok(katex), Ok(func)) = (global("katex"), global("renderMathInElement")) else {
            return false; // injected scripts not loaded yet: retry
        };
        if katex.is_undefined() || func.is_undefined() {
            return false;
        }
        let Ok(func) = wasm_bindgen::JsCast::dyn_into::<Function>(func) else {
            return false;
        };
        // The element can be gone already (user navigated on); nothing
        // to do then, so stop retrying.
        let Some(element) = document().get_element_by_id(id).map(JsValue::from) else {
            return true;
        };
        if let Ok(options) = JSON::parse(OPTIONS_JSON) {
            _ = func.call2(&window, &element, &options);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{
        autolink_bare_urls, fence_run, is_link_destination, render_html, trim_trailing_punctuation,
    };

    /// Asserts `html` holds a link to `href` (ammonia adds
    /// `rel="noopener noreferrer"`, so match on the attribute only).
    #[track_caller]
    fn assert_link(html: &str, href: &str) {
        assert!(
            html.contains(&format!(r#"href="{href}""#)),
            "expected link to {href} → html: {html}"
        );
    }

    #[test]
    fn bare_url_is_autolinked() {
        let html = render_html("See https://example.com/spec for details.");
        assert_link(&html, "https://example.com/spec");
        assert!(
            html.contains(">https://example.com/spec</a>"),
            "html: {html}"
        );
        // The trailing sentence period is not part of the URL.
        assert!(!html.contains("spec.</a>"), "html: {html}");
    }

    #[test]
    fn bare_url_at_line_start_and_end() {
        let html = render_html("https://example.com/start\n\nplain http://example.com/end");
        assert_link(&html, "https://example.com/start");
        assert_link(&html, "http://example.com/end");
    }

    #[test]
    fn bare_url_trailing_punctuation_is_not_part_of_the_link() {
        for (input, href) in [
            ("https://example.com/a.", "https://example.com/a"),
            ("https://example.com/a,", "https://example.com/a"),
            ("https://example.com/a;", "https://example.com/a"),
            ("https://example.com/a:", "https://example.com/a"),
            ("https://example.com/a!", "https://example.com/a"),
            ("https://example.com/a?", "https://example.com/a"),
            ("(see https://example.com/a)", "https://example.com/a"),
            ("https://example.com/a...)", "https://example.com/a"),
        ] {
            let html = render_html(input);
            assert!(
                html.contains(&format!(r#"href="{href}""#)),
                "input {input:?} → html: {html}"
            );
            assert!(
                !html.contains(&format!(r#"href="{input}""#)),
                "input {input:?} kept its punctuation → html: {html}"
            );
        }
    }

    #[test]
    fn bare_url_balanced_parens_are_kept() {
        let html = render_html("https://en.wikipedia.org/wiki/Foo_(bar) done");
        assert_link(&html, "https://en.wikipedia.org/wiki/Foo_(bar)");
    }

    #[test]
    fn url_in_code_span_is_untouched() {
        let html = render_html("`https://example.com/code` and ``x https://example.com/y``");
        assert!(!html.contains("<a "), "html: {html}");
        assert!(
            html.contains("<code>https://example.com/code</code>"),
            "html: {html}"
        );
    }

    #[test]
    fn url_in_code_fence_is_untouched() {
        let html =
            render_html("text\n\n```\nhttps://example.com/fenced\n```\n\nhttps://example.com/live");
        assert!(
            !html.contains(r#"href="https://example.com/fenced""#),
            "html: {html}"
        );
        assert!(
            html.contains(r#"href="https://example.com/live""#),
            "html: {html}"
        );
    }

    #[test]
    fn markdown_link_destination_is_untouched() {
        let html = render_html("[the spec](https://example.com/spec)");
        assert_eq!(html.matches("<a ").count(), 1, "html: {html}");
        assert!(html.contains(">the spec</a>"), "html: {html}");
        assert_link(&html, "https://example.com/spec");
    }

    #[test]
    fn existing_autolink_is_untouched() {
        let html = render_html("<https://example.com/already>");
        assert_eq!(html.matches("<a ").count(), 1, "html: {html}");
        assert_link(&html, "https://example.com/already");
    }

    #[test]
    fn multiple_bare_urls_in_one_string() {
        let html = render_html("https://a.example/one and https://b.example/two.");
        assert_link(&html, "https://a.example/one");
        assert_link(&html, "https://b.example/two");
    }

    #[test]
    fn autolink_does_not_break_math() {
        // The pre-pass must leave `$...$` intact for the math option.
        let html = render_html("ratio $x_i < y$, see https://example.com/math.");
        assert!(html.contains("\\(x_i &lt; y\\)"), "html: {html}");
        assert_link(&html, "https://example.com/math");
    }

    #[test]
    fn gfm_table_survives_sanitization() {
        let html = render_html("| a | b |\n|---|---|\n| 1 | 2 |");
        assert!(html.contains("<table>"), "html: {html}");
        assert!(html.contains("<td>1</td>"), "html: {html}");
    }

    #[test]
    fn bold_and_list_render() {
        let html = render_html("**bold**\n\n- one\n- two");
        assert!(html.contains("<strong>bold</strong>"), "html: {html}");
        assert!(html.contains("<li>one</li>"), "html: {html}");
    }

    #[test]
    fn raw_html_is_stripped() {
        let html = render_html("<script>alert(1)</script><img src=x onerror=alert(1)>");
        assert!(!html.contains("<script"), "html: {html}");
        assert!(!html.contains("onerror"), "html: {html}");
    }

    #[test]
    fn math_becomes_katex_delimiters_and_stays_escaped() {
        let html = render_html("inline $x_i < y$ and\n\n$$\\frac{a}{b}$$");
        assert!(html.contains("\\(x_i &lt; y\\)"), "html: {html}");
        assert!(html.contains("\\[\\frac{a}{b}\\]"), "html: {html}");
        // The formula must not be mangled by Markdown emphasis rules.
        assert!(!html.contains("<em>"), "html: {html}");
    }

    #[test]
    fn fence_run_accepts_valid_fences() {
        // Backtick and tilde markers, runs of three or more, up to three
        // leading spaces; the rest of the line (info string) is returned.
        assert_eq!(fence_run("```"), Some((b'`', 3, "")));
        assert_eq!(fence_run("~~~~"), Some((b'~', 4, "")));
        assert_eq!(fence_run("   ```"), Some((b'`', 3, "")));
        assert_eq!(fence_run("``` rust"), Some((b'`', 3, " rust")));
    }

    #[test]
    fn fence_run_rejects_non_fences() {
        // Four spaces of indentation make it an indented code block.
        assert_eq!(fence_run("    ```"), None);
        // Runs shorter than three markers are not fences.
        assert_eq!(fence_run("``"), None);
        assert_eq!(fence_run("~~"), None);
        // Ordinary text and empty lines are not fences.
        assert_eq!(fence_run("text"), None);
        assert_eq!(fence_run(""), None);
    }

    #[test]
    fn fence_stays_open_until_a_matching_bare_closing_fence() {
        // A tilde run does not close a backtick fence: the marker must match.
        let input = "```\n~~~\nhttps://example.com/a";
        assert_eq!(autolink_bare_urls(input), input);
        // An info string on the closing fence keeps the block open.
        let input = "```\n``` rust\nhttps://example.com/a";
        assert_eq!(autolink_bare_urls(input), input);
        // A shorter run does not close a longer fence.
        let input = "````\n```\nhttps://example.com/a";
        assert_eq!(autolink_bare_urls(input), input);
        // A matching bare fence closes the block; autolinking resumes.
        let input = "```\n```\nhttps://example.com/a";
        assert_eq!(
            autolink_bare_urls(input),
            "```\n```\n<https://example.com/a>"
        );
    }

    #[test]
    fn link_destination_requires_bracket_paren_before_the_url() {
        // `](` — optionally followed by spaces — before the URL.
        assert!(is_link_destination("[text](https://example.com)", 7));
        assert!(is_link_destination("[text](  https://example.com)", 9));
        // A bare parenthesized URL is not a link destination ...
        assert!(!is_link_destination("(see https://example.com)", 5));
        // ... and neither is a `(` without a preceding `]` ...
        assert!(!is_link_destination("(https://example.com)", 1));
        // ... nor a URL out in the open.
        assert!(!is_link_destination("bare https://example.com", 5));
        assert!(!is_link_destination("https://example.com", 0));
    }

    #[test]
    fn trim_trailing_punctuation_strips_exactly_the_trailing_junk() {
        // `.,;:!?` are stripped, repeatedly.
        assert_eq!(trim_trailing_punctuation("a."), 1);
        assert_eq!(trim_trailing_punctuation("a.,;:!?"), 1);
        // Balanced parens belong to the URL.
        assert_eq!(trim_trailing_punctuation("a(b)"), 4);
        // An unbalanced trailing `)` is stripped, then the balance is
        // restored and stripping stops.
        assert_eq!(trim_trailing_punctuation("a(b))"), 4);
        assert_eq!(trim_trailing_punctuation("a))"), 1);
        // Interior punctuation is untouched.
        assert_eq!(trim_trailing_punctuation("a.b(c)"), 6);
    }

    #[test]
    fn real_world_aligned_block_is_not_mangled() {
        // From the real card collection: asterisks and underscores that
        // plain `$`-delimiter scanning would have let Markdown mangle.
        let html = render_html(
            "$$\n\\begin{aligned}\n x^* &= y \\\\\n d(x, x_n) &\\to 0\n\\end{aligned}\n$$\n\nInline $e(x) = x - \\mathtt{0xffff}$ done.",
        );
        assert!(html.contains("x^* &amp;= y"), "html: {html}");
        assert!(!html.contains("<em>"), "html: {html}");
        assert!(
            html.contains("\\(e(x) = x - \\mathtt{0xffff}\\)"),
            "html: {html}"
        );
    }
}

#[cfg(test)]
mod needs_katex_tests {
    #[test]
    fn needs_katex_detects_math_and_ignores_plain_text() {
        use super::needs_katex;
        // Rendered math appears as \( or \[ delimiters.
        assert!(needs_katex("<p>inline \\(x^2\\) here</p>"));
        assert!(needs_katex("<p>display \\[a/b\\]</p>"));
        // No math → no KaTeX injection (plain dollars included).
        assert!(!needs_katex("<p>plain text</p>"));
        assert!(!needs_katex("<p>costs $100 and $50</p>"));
        assert!(!needs_katex(""));
    }
}
