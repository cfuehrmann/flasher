//! WASM entry point: mount the app to `<body>`.

#[cfg(feature = "csr")]
fn main() {
    console_error_panic_hook::set_once();
    // Remove the static loading skeleton from index.html before mounting
    // (mount_to_body appends, it does not clear <body>).
    if let Some(skeleton) = leptos::prelude::document().get_element_by_id("app-skeleton") {
        skeleton.remove();
    }
    leptos::mount::mount_to_body(flasher_leptos::App);
}

#[cfg(not(feature = "csr"))]
fn main() {}
