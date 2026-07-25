use gloo_file::futures::read_as_text;
use gloo_file::File as GlooFile;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::HtmlInputElement;

/// Reads the first file picked in a `<input type=file>` as UTF-8 text.
pub async fn read_input_file(input: &HtmlInputElement) -> Option<Result<String, String>> {
    let files = input.files()?;
    let file = files.get(0)?;
    let gloo = GlooFile::from(file);
    Some(read_as_text(&gloo).await.map_err(|e| e.to_string()))
}

/// Triggers a browser "Save As" download of `contents` as `filename`.
pub fn trigger_download(filename: &str, contents: &str) {
    let parts = js_sys::Array::new();
    parts.push(&JsValue::from_str(contents));

    let options = web_sys::BlobPropertyBag::new();
    options.set_type("application/json");
    let blob = match web_sys::Blob::new_with_str_sequence_and_options(&parts, &options) {
        Ok(b) => b,
        Err(_) => return,
    };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
        return;
    };

    let Some(window) = web_sys::window() else { return };
    let Some(document) = window.document() else { return };
    let Ok(el) = document.create_element("a") else { return };
    let Ok(anchor) = el.dyn_into::<web_sys::HtmlAnchorElement>() else {
        return;
    };
    anchor.set_href(&url);
    anchor.set_download(filename);
    if let Some(style) = anchor.dyn_ref::<web_sys::HtmlElement>().map(|e| e.style()) {
        let _ = style.set_property("display", "none");
    }
    if let Some(body) = document.body() {
        let _ = body.append_child(&anchor);
        anchor.click();
        let _ = body.remove_child(&anchor);
    }
    let _ = web_sys::Url::revoke_object_url(&url);
}
