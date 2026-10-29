//! GETs from the browser.

use chrono::{DateTime, Duration, Utc};
use std::cell::RefCell;
use std::collections::HashMap;
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

thread_local! {
    static TOKENS: RefCell<HashMap<&'static str, (String, DateTime<Utc>)>> = RefCell::new(HashMap::new());
}

/// The Planetary Computer's access token (a SAS query) for a collection's
/// files, kept while it has more than a few minutes left.
pub async fn pc_token(collection: &'static str) -> Result<String, String> {
    let fresh = |(token, expiry): &(String, DateTime<Utc>)| (*expiry - Utc::now() > Duration::minutes(5)).then(|| token.clone());
    if let Some(token) = TOKENS.with_borrow(|t| t.get(collection).and_then(fresh)) {
        return Ok(token);
    }
    let text = text(&format!("https://planetarycomputer.microsoft.com/api/sas/v1/token/{collection}")).await?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let token = v["token"].as_str().ok_or("no token in the answer")?.to_string();
    let expiry = v["msft:expiry"]
        .as_str()
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map_or_else(|| Utc::now() + Duration::minutes(30), |t| t.with_timezone(&Utc));
    TOKENS.with_borrow_mut(|t| t.insert(collection, (token.clone(), expiry)));
    Ok(token)
}

/// `text` for a URL built on the spot.
pub async fn text_owned(url: String) -> Result<String, String> {
    text(&url).await
}
