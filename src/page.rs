//! The page's address and the clipboard (see `js/page.js`).

use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/js/page.js")]
extern "C" {
    pub fn hash() -> String;
    #[wasm_bindgen(js_name = setHash)]
    pub fn set_hash(hash: &str);
    pub fn link() -> String;
    pub fn copy(text: &str) -> js_sys::Promise;
    #[wasm_bindgen(js_name = onHashChange)]
    pub fn on_hash_change(callback: &js_sys::Function);
}
