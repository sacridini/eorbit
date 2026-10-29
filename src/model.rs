//! The application's state, its downloads and the per-frame work.

use crate::archive::{self, Catalogue, Mission, Pass};
use crate::share::{self, Shared};
use crate::sky::{self, Layers};
use crate::{color, fetch, globe, page, quality};
use chrono::{DateTime, Duration, Utc};
use leptos::prelude::*;
use leptos::task::spawn_local;
use eorbit::wire::{Reply, Request};
use orbitui::aoi::{self, Shape};
use orbitui::app::build_sats;
use orbitui::catalog::{self, SatInfo};
use orbitui::clouds::Clouds;
use orbitui::geocode::{self, Place};
use orbitui::orbit::{Observer, Sat};
use orbitui::plan::{self, Plans, Status};
use orbitui::predict::{self, Acquisition, Forecast};
use orbitui::tle::{self, Source};
use orbitui::{astro, config, export, platform};
use serde_json::{Value, json};
use sgp4::Elements;
use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Arc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{MessageEvent, Worker};

/// How often the interface follows the simulated time (the globe follows
/// it every frame).
const TICK_MS: f64 = 125.0;
/// Time spent on the forecast per frame, when it cannot run in a worker.
const FORECAST_BUDGET: web_time::Duration = web_time::Duration::from_millis(12);
/// The view of the globe that shows the whole area's hemisphere.
const HOME_HEIGHT_M: f64 = 16_000_000.0;
/// A shared opportunity is found in the forecast within this many seconds.
const SHARED_SLACK_S: i64 = 120;

/// Simulated time: real time, shifted, sped up or frozen.
#[derive(Clone, Copy, Debug)]
pub struct Clock {
    real: f64,
    sim: f64,
    pub warp: f64,
    pub paused: bool,
}

impl Clock {
    pub fn live() -> Self {
        let t = js_sys::Date::now();
        Clock { real: t, sim: t, warp: 1.0, paused: false }
    }

    pub fn now_ms(&self) -> f64 {
        if self.paused { self.sim } else { self.sim + (js_sys::Date::now() - self.real) * self.warp }
    }

    pub fn now(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(self.now_ms() as i64).unwrap_or_default()
    }

    fn rebase(&mut self) {
        self.sim = self.now_ms();
        self.real = js_sys::Date::now();
    }

    pub fn set_warp(&mut self, warp: f64) {
        self.rebase();
        self.warp = warp;
        self.paused = false;
    }

    pub fn toggle(&mut self) {
        self.rebase();
        self.paused = !self.paused;
    }

    pub fn jump(&mut self, t: DateTime<Utc>) {
        self.rebase();
        self.sim = t.timestamp_millis() as f64;
    }

    pub fn is_live(&self) -> bool {
        !self.paused && self.warp == 1.0 && (self.now_ms() - js_sys::Date::now()).abs() < 2000.0
    }
}

/// Where the orbital elements come from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElementsStatus {
    pub source: Source,
    pub fetched: Option<DateTime<Utc>>,
    pub updating: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    Imagery,
    Map,
}

impl Base {
    pub fn id(self) -> &'static str {
        match self {
            Base::Imagery => "imagery",
            Base::Map => "map",
        }
    }
}

/// NASA's view of one day laid over the whole globe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sky {
    /// Days before today (UTC).
    pub back: i64,
    /// Which of archive::DAILY.
    pub layer: usize,
}

impl Sky {
    /// Yesterday's, the latest whole one, from NOAA-20 (VIIRS).
    pub const LATEST: Sky = Sky { back: 1, layer: 3 };
    /// How far back the days go.
    pub const MAX_BACK: i64 = 60;

    pub fn date(self) -> chrono::NaiveDate {
        (Utc::now() - Duration::days(self.back)).date_naive()
    }

    pub fn tiles(self) -> String {
        archive::daily_tiles(archive::DAILY[self.layer].2, self.date())
    }
}

/// What fills the page beside the list of opportunities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Globe,
    Timeline,
    Calendar,
    Revisit,
    Images,
}

impl View {
    pub const ALL: [View; 5] = [View::Globe, View::Timeline, View::Calendar, View::Revisit, View::Images];

    /// Its name in links.
    pub fn id(self) -> &'static str {
        match self {
            View::Globe => "globe",
            View::Timeline => "timeline",
            View::Calendar => "calendar",
            View::Revisit => "revisit",
            View::Images => "images",
        }
    }

    pub fn from_id(id: &str) -> Option<View> {
        View::ALL.into_iter().find(|v| v.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            View::Globe => "Globe",
            View::Timeline => "Timeline",
            View::Calendar => "Calendar",
            View::Revisit => "Revisit",
            View::Images => "Images",
        }
    }
}

/// The scenes already acquired over the area.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Archive {
    /// The area and span searched.
    key: String,
    pub loading: bool,
    pub passes: Vec<Pass>,
    /// Missions whose search failed, and why.
    pub errors: Vec<String>,
    pub since: Option<DateTime<Utc>>,
    /// Where the scenes come from.
    pub catalogue: Option<archive::Catalogue>,
}

/// How the list of opportunities is ordered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Soonest,
    Best,
}

/// An orbit drawn on the globe: whose, and around which time.
#[derive(Clone, Copy, PartialEq)]
struct Drawn {
    norad: u64,
    at: DateTime<Utc>,
}

/// The forecast worker, when the browser runs one.
#[derive(Default)]
struct Pool {
    worker: Option<Worker>,
    ready: bool,
    busy: bool,
    queued: Option<String>,
    /// The worker failed: forecasts run on the page.
    broken: bool,
}

thread_local! {
    static POOL: RefCell<Pool> = RefCell::new(Pool::default());
}

#[derive(Clone, Copy)]
pub struct Model {
    pub catalog: RwSignal<Vec<SatInfo>>,
    elements: StoredValue<Vec<Elements>>,
    pub sats: RwSignal<Arc<Vec<Sat>>>,
    pub elements_status: RwSignal<ElementsStatus>,
    pub observer: RwSignal<Observer>,
    pub forecast: RwSignal<Option<Arc<Forecast>>>,
    /// The span of the forecast (computed or in progress).
    pub window: RwSignal<Option<(DateTime<Utc>, DateTime<Utc>)>>,
    /// Share of the forecast computed, while it is.
    pub progress: RwSignal<Option<f64>>,
    job_id: StoredValue<u64>,
    /// The forecast advancing between frames, without a worker.
    job: StoredValue<Option<predict::Job>>,
    pub clouds: RwSignal<Option<Clouds>>,
    pub layers: RwSignal<Option<Layers>>,
    pub plans: RwSignal<Plans>,
    pub clock: RwSignal<Clock>,
    /// The simulated time, refreshed a few times a second.
    pub now: RwSignal<DateTime<Utc>>,
    /// The selected satellite, by NORAD id.
    pub selected: RwSignal<Option<u64>>,
    /// The opportunity being looked at.
    pub event: RwSignal<Option<Acquisition>>,
    pub days: RwSignal<f64>,
    pub min_el: f64,
    /// List opportunities the sensor cannot use (optical sensors at night).
    pub show_unusable: RwSignal<bool>,
    pub only_selected: RwSignal<bool>,
    pub order: RwSignal<Order>,
    pub view: RwSignal<View>,
    pub utc: RwSignal<bool>,
    pub picking: RwSignal<bool>,
    pub labels: RwSignal<bool>,
    pub base: RwSignal<Base>,
    /// NASA's view of a recent day over the globe, when shown.
    pub sky: RwSignal<Option<Sky>>,
    pub message: RwSignal<Option<(String, f64)>>,
    pub archive: RwSignal<Archive>,
    /// How far back the archive is searched.
    pub archive_days: RwSignal<u32>,
    /// The acquired scene shown on the globe.
    pub scene: RwSignal<Option<Pass>>,
    /// A shared opportunity, opened when the forecast holds it.
    pending: StoredValue<Option<(u64, DateTime<Utc>)>>,
    drawn: StoredValue<Option<Drawn>>,
    last_tick: StoredValue<f64>,
}

impl Model {
    pub fn new() -> Self {
        let set = tle::load_local();
        let (catalog, catalog_error) = catalog::load();
        let sats = build_sats(&set.elements, &catalog);
        let observer = config::load().unwrap_or_else(config::default_observer);
        let clock = Clock::live();
        let m = Model {
            selected: RwSignal::new(sats.first().map(|s| s.info.norad)),
            sats: RwSignal::new(Arc::new(sats)),
            catalog: RwSignal::new(catalog),
            elements: StoredValue::new(set.elements.clone()),
            elements_status: RwSignal::new(ElementsStatus { source: set.source, fetched: set.fetched, updating: false }),
            observer: RwSignal::new(observer),
            forecast: RwSignal::new(None),
            window: RwSignal::new(None),
            progress: RwSignal::new(None),
            job_id: StoredValue::new(0),
            job: StoredValue::new(None),
            clouds: RwSignal::new(None),
            layers: RwSignal::new(None),
            plans: RwSignal::new(Plans::default()),
            now: RwSignal::new(clock.now()),
            clock: RwSignal::new(clock),
            event: RwSignal::new(None),
            days: RwSignal::new(share::DEFAULT_DAYS),
            min_el: 10.0,
            show_unusable: RwSignal::new(false),
            only_selected: RwSignal::new(false),
            order: RwSignal::new(Order::Soonest),
            view: RwSignal::new(View::Globe),
            utc: RwSignal::new(false),
            picking: RwSignal::new(false),
            labels: RwSignal::new(true),
            base: RwSignal::new(Base::Imagery),
            sky: RwSignal::new(None),
            message: RwSignal::new(None),
            archive: RwSignal::new(Archive::default()),
            archive_days: RwSignal::new(90),
            scene: RwSignal::new(None),
            pending: StoredValue::new(None),
            drawn: StoredValue::new(None),
            last_tick: StoredValue::new(0.0),
        };
        if let Some(e) = catalog_error {
            m.say(e);
        }
        if set.is_stale() {
            m.refresh_elements();
        } else {
            m.fetch_missing();
        }
        m.fetch_clouds();
        m
    }

    pub fn say(self, text: impl Into<String>) {
        self.message.set(Some((text.into(), js_sys::Date::now())));
    }

    /// Index of the selected satellite (tracked).
    pub fn selected_index(self) -> Option<usize> {
        let norad = self.selected.get()?;
        self.sats.with(|s| s.iter().position(|s| s.info.norad == norad))
    }

    pub fn select(self, norad: u64) {
        self.selected.set(Some(norad));
    }

    pub fn set_time(self, t: DateTime<Utc>) {
        self.clock.update(|c| c.jump(t));
        self.now.set(t);
    }

    /// Looks at an opportunity: the clock goes to just before it.
    pub fn open_event(self, a: Acquisition) {
        if let Some(norad) = self.sats.with_untracked(|s| s.get(a.sat).map(|s| s.info.norad)) {
            self.selected.set(Some(norad));
        }
        self.set_time(a.time - Duration::seconds(90));
        self.clock.update(|c| c.set_warp(10.0));
        if self.view.get_untracked() == View::Globe {
            let obs = self.observer.get_untracked();
            globe::fly_to(obs.lat, obs.lon, 3_500_000.0, 2.0);
        }
        self.event.set(Some(a));
    }

    /// Back to the whole area of interest: no satellite, no opportunity.
    pub fn reset_view(self) {
        self.selected.set(None);
        self.event.set(None);
        self.hide_scene();
        self.picking.set(false);
        let obs = self.observer.get_untracked();
        globe::fly_to(obs.lat, obs.lon, HOME_HEIGHT_M, 1.5);
    }

    pub fn set_observer(self, obs: Observer) {
        config::save(&obs);
        globe::fly_to(obs.lat, obs.lon, 12_000_000.0, 2.0);
        self.observer.set(obs);
        self.event.set(None);
        self.hide_scene();
        self.fetch_clouds();
        self.request_forecast();
    }

    pub fn home(self) {
        let obs = self.observer.get_untracked();
        globe::fly_to(obs.lat, obs.lon, HOME_HEIGHT_M, 0.0);
    }

    /// Sets the AOI from a GeoJSON document.
    pub fn load_geojson(self, text: &str, fallback_name: &str) {
        match aoi::parse_geojson(text) {
            Ok((Shape::Area(area), name)) => {
                let km2 = area.area_km2;
                self.set_observer(Observer::with_area(name.unwrap_or_else(|| fallback_name.to_string()), area));
                self.say(format!("Area of {km2:.0} km² loaded"));
            }
            Ok((Shape::Point(lat, lon), name)) => {
                self.set_observer(Observer::new(name.unwrap_or_else(|| fallback_name.to_string()), lat, lon, 0.0));
            }
            Err(e) => self.say(e),
        }
    }

    // ---- Shared links ----

    /// Opens what a link asks for.
    pub fn apply_shared(self, shared: Shared) {
        if let Some(view) = shared.view {
            self.view.set(view);
        }
        if let Some(days) = shared.days {
            self.days.set(days);
        }
        if let Some(norad) = shared.sat {
            self.selected.set(Some(norad));
        }
        if let (Some(norad), Some(t)) = (shared.sat, shared.at) {
            let now = Utc::now();
            if t < now - Duration::seconds(SHARED_SLACK_S) {
                self.say("The shared opportunity has passed; its satellite stays selected");
            } else {
                // A forecast long enough to hold it.
                let need = (t - now).num_seconds() as f64 / 86_400.0 + 0.5;
                if need > self.days.get_untracked() {
                    let days = crate::ui::timeline::DAYS.into_iter().find(|&d| d >= need).unwrap_or(30.0);
                    self.days.set(days);
                }
                self.pending.set_value(Some((norad, t)));
            }
        }
        match shared.observer {
            Some(obs) => self.set_observer(obs),
            None => self.request_forecast(),
        }
    }

    /// The shared opportunity, once the forecast holds it.
    pub fn open_pending(self) {
        let Some((norad, t)) = self.pending.get_value() else { return };
        self.pending.set_value(None);
        let sats = self.sats.get_untracked();
        let found = self.forecast.with_untracked(|f| {
            f.as_ref()?
                .acquisitions
                .iter()
                .find(|a| {
                    sats.get(a.sat).is_some_and(|s| s.info.norad == norad) && (a.time - t).num_seconds().abs() <= SHARED_SLACK_S
                })
                .cloned()
        });
        match found {
            Some(a) => self.open_event(a),
            None => self.say("The shared opportunity is not in the forecast any more"),
        }
    }

    /// The state to put in the page's address (tracked).
    pub fn shared_state(self) -> String {
        let at = self.event.with(|e| e.as_ref().map(|a| a.time));
        self.observer.with(|obs| share::encode(obs, self.view.get(), self.selected.get(), at, self.days.get()))
    }

    /// Copies a link to what is on the screen.
    pub fn share(self) {
        page::set_hash(&untrack(|| self.shared_state()));
        let link = page::link();
        let what = if self.event.with_untracked(Option::is_some) {
            "this opportunity"
        } else if self.selected.with_untracked(Option::is_some) {
            "this area and satellite"
        } else {
            "this area"
        };
        spawn_local(async move {
            let copied = JsFuture::from(page::copy(&link)).await.ok().and_then(|v| v.as_bool()).unwrap_or(false);
            if copied {
                self.say(format!("Link copied: it opens {what}"));
            } else {
                self.say("Copy the address in the browser's bar to share this view");
            }
        });
    }

    // ---- Acquired images ----

    /// Searches the scenes acquired over the area, unless they are known.
    pub fn fetch_archive(self) {
        let obs = self.observer.get_untracked();
        let days = self.archive_days.get_untracked();
        let key = format!("{:.5},{:.5},{:.1},{days}", obs.lat, obs.lon, obs.area.as_ref().map_or(0.0, |a| a.area_km2));
        if self.archive.with_untracked(|a| a.key == key && (a.loading || a.errors.is_empty())) {
            return;
        }
        let end = Utc::now();
        let start = end - Duration::days(days.into());
        self.archive.set(Archive { key: key.clone(), loading: true, since: Some(start), ..Archive::default() });
        spawn_local(async move {
            let global = async {
                let mut catalogue = Catalogue::PlanetaryComputer;
                let mut result = search_catalogue(catalogue, &obs, start, end).await;
                // Blocked (often by a privacy extension): the other catalogue.
                if result.is_none() {
                    catalogue = Catalogue::EarthSearch;
                    result = search_catalogue(catalogue, &obs, start, end).await;
                }
                (catalogue, result)
            };
            let brazil = search_catalogue(Catalogue::BrazilDataCube, &obs, start, end);
            let ((catalogue, result), brazil) = futures::future::join(global, brazil).await;
            let daily = archive::daily(&obs, end);
            if self.archive.with_untracked(|a| a.key != key) {
                return;
            }
            let unreachable = |hosts: &str| {
                format!(
                    "the browser could not reach {hosts}. Offline, or blocked by a privacy extension (Privacy Badger, uBlock) or strict tracking protection?"
                )
            };
            let (mut scenes, mut errors) = result.unwrap_or_else(|| {
                let hosts = [Catalogue::PlanetaryComputer.host(), Catalogue::EarthSearch.host()].join(" and ");
                (Vec::new(), vec![unreachable(&hosts)])
            });
            match brazil {
                Some((more, failed)) => {
                    scenes.extend(more);
                    errors.extend(failed);
                }
                None => errors.push(unreachable(Catalogue::BrazilDataCube.host())),
            }
            scenes.extend(daily);
            self.archive.set(Archive {
                key,
                loading: false,
                passes: archive::passes(scenes),
                errors,
                since: Some(start),
                catalogue: Some(catalogue),
            });
        });
    }

    /// Saves a scene's file, asking first for its access token if it needs one.
    pub fn download(self, file: archive::Download) {
        let Some(collection) = file.sign else {
            page::save(&file.url);
            return;
        };
        spawn_local(async move {
            match fetch::pc_token(collection).await {
                Ok(token) => {
                    self.say("Downloading the whole scene: a large file, it takes a while");
                    page::save(&format!("{}?{token}", file.url));
                }
                Err(e) => self.say(format!("The Planetary Computer did not open the file: {e}")),
            }
        });
    }

    /// A pass's satellite, and its camera where it has several worth naming.
    pub fn pass_name(self, pass: &Pass) -> String {
        let name = self.platform_sat(&pass.platform).map_or_else(|| pass.platform.clone(), |i| i.name.to_string());
        match pass.instrument.as_ref().filter(|_| pass.mission.names_instrument()) {
            Some(camera) => format!("{name} {camera}"),
            None => name,
        }
    }

    /// The satellite of a catalogue's platform, when the fleet has it.
    pub fn platform_sat(self, platform: &str) -> Option<SatInfo> {
        self.sats.with_untracked(|s| s.iter().find(|s| archive::same_platform(platform, s.info.name)).map(|s| s.info))
    }

    pub fn platform_color(self, platform: &str) -> String {
        self.platform_sat(platform).map_or_else(|| "#e9f0f8".to_string(), |info| color::css(info.color))
    }

    /// Drapes an acquired scene over the globe and flies to it.
    pub fn show_scene(self, pass: Pass) {
        self.view.set(View::Globe);
        let [west, south, east, north] = pass.bbox();
        globe::fly_to_box(west, south, east, north);
        let color = self.platform_color(&pass.platform);
        let key = pass.key();
        self.scene.set(Some(pass.clone()));
        spawn_local(async move {
            let layers = futures::future::join_all(pass.scenes.iter().map(|s| async move {
                let tiles = match s.tilejson.as_deref() {
                    _ if s.tiles.is_some() => s.tiles.clone(),
                    Some(url) => fetch::text(url)
                        .await
                        .ok()
                        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                        .and_then(|v| v["tiles"][0].as_str().map(str::to_string)),
                    None => None,
                };
                let rings: Vec<Vec<f64>> = s.rings.iter().map(|r| r.iter().flat_map(|&(lat, lon)| [lon, lat]).collect()).collect();
                json!({
                    "tiles": tiles,
                    "image": s.snapshot,
                    // GIBS draws the daily views down to ~250 m.
                    "max": if s.mission == Mission::Daily { 9 } else { 14 },
                    "credit": if s.mission == Mission::Daily { "NASA GIBS" } else { "Microsoft Planetary Computer" },
                    "west": s.bbox[0],
                    "south": s.bbox[1],
                    "east": s.bbox[2],
                    "north": s.bbox[3],
                    "rings": rings,
                })
            }))
            .await;
            if self.scene.with_untracked(|p| p.as_ref().map(Pass::key)) != Some(key) {
                return;
            }
            if layers.iter().all(|l| l["tiles"].is_null() && l["image"].is_null()) {
                self.say("Only the image's outline: its catalogue does not draw it on maps");
            }
            globe::set_scenes(&Value::Array(layers).to_string(), &color);
        });
    }

    pub fn hide_scene(self) {
        if self.scene.with_untracked(Option::is_some) {
            self.scene.set(None);
            globe::set_scenes("[]", "");
        }
    }

    // ---- Acquisition plans ----

    /// Reads an ESA acquisition plan (KML).
    pub fn load_plan(self, name: &str, text: &str) {
        match plan::parse_kml(text, name) {
            Ok(file) => {
                self.say(format!(
                    "Plan of {} loaded: {} segments, {} to {}",
                    file.sat,
                    file.segments.len(),
                    file.start.format("%d %b"),
                    file.end.format("%d %b")
                ));
                self.plans.update(|p| p.add(file));
            }
            Err(e) => self.say(format!("{name}: {e}")),
        }
    }

    pub fn remove_plan(self, name: &str) {
        self.plans.update(|p| p.files.retain(|f| f.name != name));
    }

    /// Whether the mission plans this acquisition (tracked).
    pub fn plan_status(self, a: &Acquisition) -> Status {
        let Some(short) = self.sats.with(|s| s.get(a.sat).map(|s| s.info.short)) else { return Status::Unknown };
        self.observer.with(|obs| self.plans.with(|p| p.status_for(short, a.time, obs)))
    }

    // ---- Forecast ----

    pub fn request_forecast(self) {
        let now = self.clock.get_untracked().now();
        let start = now - Duration::hours(1);
        let end = now + Duration::seconds((self.days.get_untracked() * 86_400.0) as i64);
        let job = self.job_id.get_value() + 1;
        self.job_id.set_value(job);
        self.window.set(Some((start, end)));
        self.forecast.set(None);
        self.progress.set(Some(0.0));
        let sats = self.sats.get_untracked();
        let obs = self.observer.get_untracked();
        let request = Request::new(job, &sats, &obs, start, end, self.min_el);
        let sent = serde_json::to_string(&request).is_ok_and(|text| self.send_to_worker(text));
        let local = (!sent).then(|| predict::Job::new(sats, obs, start, end, self.min_el));
        self.job.set_value(local);
    }

    /// Hands a forecast to the worker; false when there is none.
    fn send_to_worker(self, request: String) -> bool {
        POOL.with_borrow_mut(|pool| {
            if pool.broken {
                return false;
            }
            // A forecast for an older request: start over.
            if pool.busy {
                if let Some(w) = pool.worker.take() {
                    w.terminate();
                }
                pool.ready = false;
                pool.busy = false;
            }
            if pool.worker.is_none() {
                match self.spawn_worker() {
                    Some(w) => pool.worker = Some(w),
                    None => {
                        pool.broken = true;
                        return false;
                    }
                }
            }
            match &pool.worker {
                Some(w) if pool.ready => {
                    w.post_message(&JsValue::from_str(&request)).ok();
                    pool.busy = true;
                }
                _ => pool.queued = Some(request),
            }
            true
        })
    }

    fn spawn_worker(self) -> Option<Worker> {
        let worker = Worker::new("./worker_loader.js").ok()?;
        let on_message = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
            let Some(reply) = e.data().as_string().and_then(|t| serde_json::from_str::<Reply>(&t).ok()) else { return };
            match reply {
                Reply::Ready => POOL.with_borrow_mut(|p| {
                    p.ready = true;
                    if let (Some(w), Some(q)) = (&p.worker, p.queued.take()) {
                        w.post_message(&JsValue::from_str(&q)).ok();
                        p.busy = true;
                    }
                }),
                Reply::Progress { job, share } if job == self.job_id.get_value() => self.progress.set(Some(share)),
                Reply::Progress { .. } => {}
                Reply::Done { job, forecast } => {
                    POOL.with_borrow_mut(|p| p.busy = false);
                    if job == self.job_id.get_value() {
                        self.progress.set(None);
                        self.forecast.set(Some(Arc::new(forecast)));
                    }
                }
            }
        });
        worker.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();
        let on_error = Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
            POOL.with_borrow_mut(|p| {
                p.broken = true;
                p.worker = None;
                p.busy = false;
            });
            self.request_forecast();
        });
        worker.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        on_error.forget();
        Some(worker)
    }

    /// Advances a forecast running on the page.
    fn step_forecast(self) {
        let mut out = None;
        self.job.update_value(|job| {
            if let Some(j) = job {
                out = Some((j.step(FORECAST_BUDGET), j.progress()));
            }
        });
        match out {
            Some((Some(fc), _)) => {
                self.job.set_value(None);
                self.progress.set(None);
                self.forecast.set(Some(Arc::new(fc)));
            }
            Some((None, p)) => self.progress.set(Some(p)),
            None => {}
        }
    }

    pub fn cloud_at(self, t: DateTime<Utc>) -> Option<f64> {
        self.clouds.with(|c| c.as_ref().and_then(|c| c.at(t)))
    }

    /// The quality score of an opportunity (tracked).
    pub fn score(self, a: &Acquisition) -> f64 {
        let Some(kind) = self.sats.with(|s| s.get(a.sat).map(|s| s.info.kind)) else { return 0.0 };
        quality::score(a, kind, self.cloud_at(a.time))
    }

    /// Opportunities from now on under the list's filters (tracked).
    pub fn upcoming(self, limit: usize) -> Vec<Acquisition> {
        self.opportunities(self.order.get(), limit)
    }

    pub fn opportunities(self, order: Order, limit: usize) -> Vec<Acquisition> {
        let Some(fc) = self.forecast.get() else { return Vec::new() };
        let now = self.now.get();
        let unusable = self.show_unusable.get();
        let only = self.only_selected.get().then(|| self.selected_index()).flatten();
        let sats = self.sats.get();
        let list = fc
            .acquisitions
            .iter()
            .filter(|a| a.time >= now - Duration::minutes(2))
            .filter(|a| only.is_none_or(|i| a.sat == i))
            .filter(|a| sats.get(a.sat).is_some_and(|s| unusable || a.usable(s.info.kind)));
        match order {
            Order::Soonest => list.take(limit).cloned().collect(),
            Order::Best => {
                let mut scored: Vec<(f64, Acquisition)> = list.map(|a| (self.score(a), a.clone())).collect();
                scored.sort_by(|a, b| b.0.total_cmp(&a.0));
                scored.into_iter().take(limit).map(|(_, a)| a).collect()
            }
        }
    }

    /// Downloads the upcoming opportunities as CSV or iCalendar.
    pub fn export(self, calendar: bool) {
        if self.forecast.get_untracked().is_none() {
            self.say("The forecast is still being computed");
            return;
        }
        let list = untrack(|| self.opportunities(Order::Soonest, usize::MAX));
        let refs: Vec<&Acquisition> = list.iter().collect();
        let sats = self.sats.get_untracked();
        let obs = self.observer.get_untracked();
        let result = self.plans.with_untracked(|plans| {
            self.clouds.with_untracked(|clouds| {
                let sheet = export::Sheet {
                    app: "eorbit",
                    sats: &sats,
                    observer: &obs,
                    clouds: clouds.as_ref(),
                    plans,
                    now: self.clock.get_untracked().now(),
                };
                let (ext, text) = if calendar { ("ics", export::ics(&sheet, &refs)) } else { ("csv", export::csv(&sheet, &refs)) };
                platform::deliver(&export::file_name(&sheet, ext), &text)
            })
        });
        match result {
            Ok(name) => self.say(format!("{} opportunities saved to {name}", refs.len())),
            Err(e) => self.say(format!("Export failed: {e}")),
        }
    }

    // ---- Per frame ----

    /// Starts the loop that moves the globe and advances the forecast.
    pub fn run(self) {
        use std::rc::Rc;

        type Frame = Closure<dyn FnMut()>;
        let slot: Rc<RefCell<Option<Frame>>> = Rc::new(RefCell::new(None));
        let again = Rc::clone(&slot);
        *slot.borrow_mut() = Some(Closure::new(move || {
            self.frame();
            if let (Some(w), Some(f)) = (web_sys::window(), again.borrow().as_ref()) {
                w.request_animation_frame(f.as_ref().unchecked_ref()).ok();
            }
        }));
        if let (Some(w), Some(f)) = (web_sys::window(), slot.borrow().as_ref()) {
            w.request_animation_frame(f.as_ref().unchecked_ref()).ok();
        }
    }

    fn frame(self) {
        let clock = self.clock.get_untracked();
        let now = clock.now();
        let sats = self.sats.get_untracked();
        let on_globe = self.view.get_untracked() == View::Globe;
        if on_globe {
            globe::update(clock.now_ms(), &globe::positions(&sats, now));
        }

        let real = js_sys::Date::now();
        if real - self.last_tick.get_value() >= TICK_MS {
            self.last_tick.set_value(real);
            self.now.set(now);
            if self.message.with_untracked(|m| m.as_ref().is_some_and(|(_, at)| real - at > 6000.0)) {
                self.message.set(None);
            }
        }

        // A new forecast when the time leaves the one computed.
        let outside = self.window.get_untracked().is_none_or(|(a, b)| now < a || now > b - Duration::hours(6));
        if outside && self.progress.get_untracked().is_none() {
            self.request_forecast();
        }
        self.step_forecast();
        if on_globe {
            self.draw_orbit(&sats, now);
        }
    }

    /// Redraws the selected satellite's orbit once the time has moved on.
    fn draw_orbit(self, sats: &[Sat], now: DateTime<Utc>) {
        let Some(sat) = self.selected.get_untracked().and_then(|n| sats.iter().find(|s| s.info.norad == n)) else {
            if self.drawn.get_value().is_some() {
                globe::clear_orbit();
                self.drawn.set_value(None);
            }
            return;
        };
        let period = Duration::seconds((sat.period_minutes() * 60.0) as i64);
        let fresh = self.drawn.get_value().is_some_and(|d| d.norad == sat.info.norad && (now - d.at).abs() < period / 6);
        if !fresh {
            globe::orbit(sat, now - period / 4, now + period);
            self.drawn.set_value(Some(Drawn { norad: sat.info.norad, at: now }));
        }
    }

    /// Forces the orbit to be drawn again (the fleet changed).
    pub fn redraw_orbit(self) {
        self.drawn.set_value(None);
    }

    // ---- Downloads ----

    fn install(self, elements: Vec<Elements>, source: Source, fetched: Option<DateTime<Utc>>) {
        self.elements.set_value(elements);
        self.elements_status.set(ElementsStatus { source, fetched, updating: false });
        self.rebuild();
    }

    /// Rebuilds the fleet from the catalog and the elements.
    fn rebuild(self) {
        let sats = self.elements.with_value(|e| build_sats(e, &self.catalog.get_untracked()));
        if self.selected.get_untracked().is_some_and(|n| !sats.iter().any(|s| s.info.norad == n)) {
            self.selected.set(None);
        }
        self.event.set(None);
        self.sats.set(Arc::new(sats));
        self.redraw_orbit();
        self.request_forecast();
    }

    pub fn reload_catalog(self) {
        let (catalog, error) = catalog::load();
        self.catalog.set(catalog);
        if let Some(e) = error {
            self.say(e);
        }
        self.rebuild();
        self.fetch_missing();
    }

    /// Downloads the Celestrak groups, then the satellites they lack.
    pub fn refresh_elements(self) {
        if self.elements_status.get_untracked().updating {
            return;
        }
        self.elements_status.update(|s| s.updating = true);
        spawn_local(async move {
            let wanted: HashSet<u64> = self.catalog.get_untracked().iter().map(|s| s.norad).collect();
            let answers = futures::future::join_all(tle::GROUPS.iter().map(|g| fetch::text_owned(tle::group_url(g)))).await;
            let mut found: Vec<Elements> = Vec::new();
            let mut error = None;
            for answer in answers {
                match answer {
                    Ok(text) => absorb(&mut found, tle::parse(&text), &wanted),
                    Err(e) => error = Some(e),
                }
            }
            let have: HashSet<u64> = found.iter().map(|e| e.norad_id).collect();
            let missing: Vec<u64> = wanted.iter().copied().filter(|id| !have.contains(id)).collect();
            let answers = futures::future::join_all(missing.iter().map(|&id| fetch::text_owned(tle::catnr_url(id)))).await;
            for text in answers.into_iter().flatten() {
                absorb(&mut found, tle::parse(&text), &wanted);
            }
            if found.is_empty() {
                self.elements_status.update(|s| s.updating = false);
                let why = error.unwrap_or_else(|| "no elements".into());
                self.say(format!("Could not update the orbital elements ({why})"));
                return;
            }
            let set = tle::store(found);
            self.install(set.elements, set.source, set.fetched);
        });
    }

    /// Downloads elements only for satellites that have none.
    fn fetch_missing(self) {
        let have: HashSet<u64> = self.elements.with_value(|e| e.iter().map(|e| e.norad_id).collect());
        let missing: Vec<u64> =
            self.catalog.get_untracked().iter().map(|s| s.norad).filter(|id| !have.contains(id)).collect();
        if missing.is_empty() || self.elements_status.get_untracked().updating {
            return;
        }
        self.elements_status.update(|s| s.updating = true);
        spawn_local(async move {
            let wanted: HashSet<u64> = missing.iter().copied().collect();
            let answers = futures::future::join_all(missing.iter().map(|&id| fetch::text_owned(tle::catnr_url(id)))).await;
            let mut found = self.elements.get_value();
            for text in answers.into_iter().flatten() {
                absorb(&mut found, tle::parse(&text), &wanted);
            }
            let set = tle::store(found);
            self.install(set.elements, set.source, set.fetched);
        });
    }

    fn fetch_clouds(self) {
        let obs = self.observer.get_untracked();
        let at = (obs.lat, obs.lon);
        if self.clouds.with_untracked(|c| c.as_ref().is_some_and(|c| !c.is_stale(at))) {
            return;
        }
        spawn_local(async move {
            match fetch::text(&sky::url(at.0, at.1)).await.and_then(|t| sky::parse(&t, at)) {
                Ok((clouds, layers)) if self.observer.with_untracked(|o| (o.lat, o.lon) == at) => {
                    self.clouds.set(Some(clouds));
                    self.layers.set(Some(layers));
                }
                Ok(_) => {}
                Err(e) => self.say(format!("No cloud forecast ({e})")),
            }
        });
    }

    /// Places matching a name, best first.
    pub async fn search(query: String) -> Result<Vec<Place>, String> {
        let languages = geocode::languages();
        let answers = futures::future::join_all(languages.iter().map(|l| fetch::text_owned(geocode::url(&query, l)))).await;
        let mut places = Vec::new();
        let mut error = None;
        for answer in answers {
            match answer.and_then(|t| geocode::parse(&t)) {
                Ok(found) => geocode::merge(&mut places, found),
                Err(e) => error = Some(e),
            }
        }
        geocode::rank(&mut places);
        match (places.is_empty(), error) {
            (true, Some(e)) => Err(e),
            _ => Ok(places),
        }
    }
}

/// One catalogue's scenes over the area and the missions that failed; None
/// when the browser reached none of its searches.
async fn search_catalogue(
    catalogue: Catalogue,
    obs: &Observer,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Option<(Vec<archive::Scene>, Vec<String>)> {
    let searches = catalogue.missions().iter().map(|&mission| {
        let url = archive::url(catalogue, mission, obs, start, end);
        async move { (mission, fetch::text_owned(url).await) }
    });
    let answers = futures::future::join_all(searches).await;
    // Only network errors (no HTTP status): the requests never went out.
    if answers.iter().all(|(_, a)| a.as_ref().is_err_and(|e| !e.starts_with("HTTP"))) {
        return None;
    }
    let mut scenes = Vec::new();
    let mut errors = Vec::new();
    for (mission, answer) in answers {
        match answer.and_then(|text| archive::parse(&text, mission, obs)) {
            Ok(found) => scenes.extend(found),
            Err(e) => errors.push(format!("{}: {e}", mission.label())),
        }
    }
    Some((scenes, errors))
}

fn absorb(found: &mut Vec<Elements>, more: Vec<Elements>, wanted: &HashSet<u64>) {
    for e in more {
        if wanted.contains(&e.norad_id) && !found.iter().any(|f| f.norad_id == e.norad_id) {
            found.push(e);
        }
    }
}

/// Elevation of the Sun at the AOI.
pub fn sun_elevation(obs: &Observer, t: DateTime<Utc>) -> f64 {
    astro::sun_elevation(obs.lat, obs.lon, astro::subsolar_point(t))
}
