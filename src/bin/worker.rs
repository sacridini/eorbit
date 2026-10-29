//! The forecast worker: predicts off the page's thread, reporting progress.

use eorbit::wire::{Reply, Request};
use orbitui::predict::Job;
use std::sync::Arc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{DedicatedWorkerGlobalScope, MessageEvent};

/// Work between two progress reports.
const SLICE: web_time::Duration = web_time::Duration::from_millis(120);

fn post(scope: &DedicatedWorkerGlobalScope, reply: &Reply) {
    if let Ok(text) = serde_json::to_string(reply) {
        scope.post_message(&JsValue::from_str(&text)).ok();
    }
}

fn main() {
    console_error_panic_hook::set_once();
    let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let me = scope.clone();
    let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
        let Some(request) = e.data().as_string().and_then(|t| serde_json::from_str::<Request>(&t).ok()) else { return };
        let (sats, observer) = request.build();
        let mut job = Job::new(Arc::new(sats), observer, request.start, request.end, request.min_el);
        loop {
            if let Some(forecast) = job.step(SLICE) {
                post(&me, &Reply::Done { job: request.job, forecast });
                break;
            }
            post(&me, &Reply::Progress { job: request.job, share: job.progress() });
        }
    });
    scope.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    on_message.forget();
    post(&scope, &Reply::Ready);
}
