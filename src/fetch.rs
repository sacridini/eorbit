//! GETs from the browser.

use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::Response;

fn js_error(e: JsValue) -> String {
    e.as_string().unwrap_or_else(|| format!("{e:?}"))
}

pub async fn text(url: &str) -> Result<String, String> {
    let window = web_sys::window().ok_or("no window")?;
    let response = JsFuture::from(window.fetch_with_str(url)).await.map_err(js_error)?;
    let response: Response = response.dyn_into().map_err(js_error)?;
    if !response.ok() {
        return Err(format!("HTTP {} for {url}", response.status()));
    }
    let text = JsFuture::from(response.text().map_err(js_error)?).await.map_err(js_error)?;
    text.as_string().ok_or_else(|| "response is not text".into())
}

/// The text of a file the user chose or dropped.
pub async fn file(file: web_sys::File) -> Result<String, String> {
    let text = JsFuture::from(file.text()).await.map_err(js_error)?;
    text.as_string().ok_or_else(|| "file is not text".into())
}

/// `text` for a URL built on the spot.
pub async fn text_owned(url: String) -> Result<String, String> {
    text(&url).await
}
