//! Images already acquired over the area, from the STAC catalogue of
//! Microsoft's Planetary Computer, which also renders their previews and
//! their map tiles.

use chrono::{DateTime, SecondsFormat, Utc};
use orbitui::aoi;
use orbitui::orbit::Observer;
use serde_json::{Value, json};

pub const SEARCH: &str = "https://planetarycomputer.microsoft.com/api/stac/v1/search";
/// The most scenes asked of each mission: a search keeps its latest ones.
const LIMIT: usize = 250;
/// Scenes of one overflight (tiles, frames) are this close in time.
const SAME_PASS_S: i64 = 180;
/// A shared polygon is thinned to this many vertices per ring.
const MAX_VERTICES: usize = 400;
/// Side of the rendered previews, in pixels.
const PREVIEW_PX: u32 = 360;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mission {
    Sentinel2,
    Landsat,
    Sentinel1,
}

impl Mission {
    pub const ALL: [Mission; 3] = [Mission::Sentinel2, Mission::Landsat, Mission::Sentinel1];

    pub fn collection(self) -> &'static str {
        match self {
            Mission::Sentinel2 => "sentinel-2-l2a",
            Mission::Landsat => "landsat-c2-l2",
            Mission::Sentinel1 => "sentinel-1-grd",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Mission::Sentinel2 => "Sentinel-2",
            Mission::Landsat => "Landsat",
            Mission::Sentinel1 => "Sentinel-1",
        }
    }

    /// Whether its scenes are photographs, which clouds spoil.
    pub fn optical(self) -> bool {
        self != Mission::Sentinel1
    }
}

/// One scene: a tile (Sentinel-2), a path/row (Landsat) or a frame
/// (Sentinel-1).
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub id: String,
    pub mission: Mission,
    pub platform: String,
    pub time: DateTime<Utc>,
    /// Cloud over the whole scene, in percent (optical scenes).
    pub cloud: Option<f64>,
    pub preview: Option<String>,
    pub tilejson: Option<String>,
    /// The scene on the Planetary Computer's map.
    pub page: Option<String>,
    /// Footprint rings, (lat, lon).
    pub rings: Vec<Vec<(f64, f64)>>,
    /// West, south, east, north.
    pub bbox: [f64; 4],
    /// Whether the footprint holds the area's centre.
    pub over_center: bool,
}

/// One overflight: the scenes taken together.
#[derive(Clone, Debug, PartialEq)]
pub struct Pass {
    pub mission: Mission,
    pub platform: String,
    pub time: DateTime<Utc>,
    pub scenes: Vec<Scene>,
}

impl Pass {
    /// The scene over the area's centre, else the first.
    pub fn main(&self) -> &Scene {
        self.scenes.iter().find(|s| s.over_center).unwrap_or(&self.scenes[0])
    }

    pub fn cloud(&self) -> Option<f64> {
        self.main().cloud
    }

    pub fn key(&self) -> String {
        format!("{}@{}", self.platform, self.time.timestamp())
    }

    /// Whether the scenes reach the area's centre (else only its edge).
    pub fn over_center(&self) -> bool {
        self.scenes.iter().any(|s| s.over_center)
    }

    pub fn bbox(&self) -> [f64; 4] {
        self.scenes.iter().fold([180.0_f64, 90.0, -180.0, -90.0], |b, s| {
            [b[0].min(s.bbox[0]), b[1].min(s.bbox[1]), b[2].max(s.bbox[2]), b[3].max(s.bbox[3])]
        })
    }

    /// Clear enough to use: optical with little cloud, or radar.
    pub fn clear(&self, max_cloud: f64) -> bool {
        !self.mission.optical() || self.cloud().is_some_and(|c| c <= max_cloud)
    }
}

/// Whether a catalogue's platform ("sentinel-2c", "landsat-9") is the
/// satellite named `name` ("Sentinel-2C", "Landsat 9").
pub fn same_platform(platform: &str, name: &str) -> bool {
    let norm = |s: &str| s.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect::<String>();
    norm(platform) == norm(name)
}

/// The search for one mission's scenes over the area, latest first.
pub fn body(mission: Mission, obs: &Observer, start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    let at = |t: DateTime<Utc>| t.to_rfc3339_opts(SecondsFormat::Secs, true);
    json!({
        "collections": [mission.collection()],
        "intersects": geometry(obs),
        "datetime": format!("{}/{}", at(start), at(end)),
        "limit": LIMIT,
        "sortby": [{ "field": "datetime", "direction": "desc" }],
    })
    .to_string()
}

fn geometry(obs: &Observer) -> Value {
    let Some(area) = &obs.area else { return json!({ "type": "Point", "coordinates": [obs.lon, obs.lat] }) };
    let polygons: Vec<Value> = area
        .rings
        .iter()
        .filter(|r| r.len() >= 3)
        .map(|ring| {
            let step = ring.len().div_ceil(MAX_VERTICES);
            let mut coords: Vec<[f64; 2]> = ring.iter().step_by(step).map(|&(lat, lon)| [lon, lat]).collect();
            if coords.first() != coords.last() {
                coords.push(coords[0]);
            }
            json!([coords])
        })
        .collect();
    json!({ "type": "MultiPolygon", "coordinates": polygons })
}

/// The scenes of a search's answer.
pub fn parse(text: &str, mission: Mission, obs: &Observer) -> Result<Vec<Scene>, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("catalogue: {e}"))?;
    let features = v["features"].as_array().ok_or("catalogue: no scenes in the answer")?;
    Ok(features.iter().filter_map(|f| scene(f, mission, obs)).collect())
}

fn scene(f: &Value, mission: Mission, obs: &Observer) -> Option<Scene> {
    let p = &f["properties"];
    let time = DateTime::parse_from_rfc3339(p["datetime"].as_str()?).ok()?.with_timezone(&Utc);
    let asset = |k: &str| f["assets"][k]["href"].as_str().map(str::to_string);
    let rings = rings(&f["geometry"]);
    let bbox = match f["bbox"].as_array().map(|b| b.iter().filter_map(Value::as_f64).collect::<Vec<_>>()) {
        Some(b) if b.len() == 4 => [b[0], b[1], b[2], b[3]],
        _ => rings.iter().flatten().fold([180.0_f64, 90.0, -180.0, -90.0], |b, &(lat, lon)| {
            [b[0].min(lon), b[1].min(lat), b[2].max(lon), b[3].max(lat)]
        }),
    };
    Some(Scene {
        id: f["id"].as_str()?.to_string(),
        mission,
        platform: p["platform"].as_str().unwrap_or(mission.label()).to_string(),
        time,
        cloud: p["eo:cloud_cover"].as_f64(),
        preview: asset("rendered_preview").map(|u| format!("{u}&max_size={PREVIEW_PX}")),
        tilejson: asset("tilejson"),
        page: f["links"]
            .as_array()
            .and_then(|links| links.iter().find(|l| l["rel"] == "preview"))
            .and_then(|l| l["href"].as_str())
            .map(str::to_string),
        over_center: aoi::in_rings(&rings, obs.lat, obs.lon),
        rings,
        bbox,
    })
}

/// The outer rings of a (multi)polygon, as (lat, lon).
fn rings(g: &Value) -> Vec<Vec<(f64, f64)>> {
    let ring = |r: &Value| -> Vec<(f64, f64)> {
        r.as_array()
            .map(|pts| pts.iter().filter_map(|p| Some((p[1].as_f64()?, p[0].as_f64()?))).collect())
            .unwrap_or_default()
    };
    let all: Vec<Vec<(f64, f64)>> = match g["type"].as_str() {
        Some("Polygon") => g["coordinates"].get(0).map(ring).into_iter().collect(),
        Some("MultiPolygon") => {
            g["coordinates"].as_array().map(|ps| ps.iter().filter_map(|p| p.get(0).map(ring)).collect()).unwrap_or_default()
        }
        _ => Vec::new(),
    };
    all.into_iter().filter(|r| r.len() >= 3).collect()
}

/// Scenes gathered by overflight, latest first.
pub fn passes(mut scenes: Vec<Scene>) -> Vec<Pass> {
    scenes.sort_by_key(|s| std::cmp::Reverse(s.time));
    let mut out: Vec<Pass> = Vec::new();
    for s in scenes {
        let same = out.iter_mut().find(|p| {
            p.platform.eq_ignore_ascii_case(&s.platform) && (p.time - s.time).num_seconds().abs() <= SAME_PASS_S
        });
        match same {
            Some(p) => p.scenes.push(s),
            None => out.push(Pass { mission: s.mission, platform: s.platform.clone(), time: s.time, scenes: vec![s] }),
        }
    }
    out
}
