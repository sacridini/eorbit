//! Images already acquired over the area, from STAC catalogues:
//!
//! - Microsoft's Planetary Computer (Sentinel-2, Landsat, Sentinel-1), which
//!   also renders previews, map tiles and cut-outs of the area;
//! - Element 84's Earth Search on AWS, which stands in for it when the
//!   browser cannot reach it (privacy extensions block it);
//! - INPE's Brazil Data Cube (CBERS-4, CBERS-4A, Amazonia-1).
//!
//! NASA's daily views of the whole Earth (MODIS, VIIRS) come from GIBS and
//! Worldview, by date: no search is needed.

use chrono::{DateTime, Duration, NaiveDate, NaiveTime, SecondsFormat, Utc};
use orbitui::aoi;
use orbitui::orbit::Observer;
use serde_json::{Value, json};

/// The most scenes asked of each mission: a search keeps its latest ones.
const LIMIT: usize = 250;
/// Scenes of one overflight (tiles, frames) are this close in time.
const SAME_PASS_S: i64 = 180;
/// The area's polygon is thinned to this many vertices per ring, to keep
/// the search's address short.
const MAX_VERTICES: usize = 80;
/// Side of the rendered previews, in pixels.
const PREVIEW_PX: u32 = 360;
/// Largest side of a cut-out of the area, in pixels.
const CUT_PX: u32 = 4096;
/// Half the side of the cut-out around a point, in degrees of latitude.
const POINT_HALF_DEG: f64 = 0.1;
const PC_DATA: &str = "https://planetarycomputer.microsoft.com/api/data/v1";

/// A STAC catalogue the scenes are searched in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Catalogue {
    PlanetaryComputer,
    EarthSearch,
    BrazilDataCube,
}

impl Catalogue {
    pub fn host(self) -> &'static str {
        match self {
            Catalogue::PlanetaryComputer => "planetarycomputer.microsoft.com",
            Catalogue::EarthSearch => "earth-search.aws.element84.com",
            Catalogue::BrazilDataCube => "data.inpe.br",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Catalogue::PlanetaryComputer => "the Planetary Computer",
            Catalogue::EarthSearch => "Earth Search (AWS)",
            Catalogue::BrazilDataCube => "INPE's Brazil Data Cube",
        }
    }

    fn search(self) -> &'static str {
        match self {
            Catalogue::PlanetaryComputer => "https://planetarycomputer.microsoft.com/api/stac/v1/search",
            Catalogue::EarthSearch => "https://earth-search.aws.element84.com/v1/search",
            Catalogue::BrazilDataCube => "https://data.inpe.br/bdc/stac/v1/search",
        }
    }

    /// The missions it holds (Earth Search's Sentinel-1 has no open files).
    pub fn missions(self) -> &'static [Mission] {
        match self {
            Catalogue::PlanetaryComputer => &[Mission::Sentinel2, Mission::Landsat, Mission::Sentinel1],
            Catalogue::EarthSearch => &[Mission::Sentinel2, Mission::Landsat],
            Catalogue::BrazilDataCube => &[Mission::Cbers, Mission::Amazonia],
        }
    }

    /// The collections of a mission, comma separated.
    fn collections(self, mission: Mission) -> &'static str {
        match (self, mission) {
            (Catalogue::BrazilDataCube, Mission::Cbers) => {
                "CB4-MUX-L4-DN-1,CB4-WFI-L4-DN-1,CB4A-MUX-L4-DN-1,CB4A-WFI-L4-DN-1,CB4A-WPM-L4-DN-1"
            }
            (Catalogue::BrazilDataCube, _) => "AMZ1-WFI-L4-DN-1",
            (_, Mission::Sentinel2) => "sentinel-2-l2a",
            (_, Mission::Landsat) => "landsat-c2-l2",
            (_, _) => "sentinel-1-grd",
        }
    }

    /// Latest first, where the catalogue sorts (the Data Cube does not).
    fn sort(self) -> &'static str {
        match self {
            Catalogue::PlanetaryComputer => "&sortby=-datetime",
            Catalogue::EarthSearch => "&sortby=-properties.datetime",
            Catalogue::BrazilDataCube => "",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mission {
    Sentinel2,
    Landsat,
    Sentinel1,
    Cbers,
    Amazonia,
    /// NASA's daily views (MODIS, VIIRS).
    Daily,
}

impl Mission {
    pub const ALL: [Mission; 6] =
        [Mission::Sentinel2, Mission::Landsat, Mission::Sentinel1, Mission::Cbers, Mission::Amazonia, Mission::Daily];

    pub fn label(self) -> &'static str {
        match self {
            Mission::Sentinel2 => "Sentinel-2",
            Mission::Landsat => "Landsat",
            Mission::Sentinel1 => "Sentinel-1",
            Mission::Cbers => "CBERS-4/4A",
            Mission::Amazonia => "Amazonia-1",
            Mission::Daily => "MODIS/VIIRS",
        }
    }

    /// Whether its scenes are photographs, which clouds spoil.
    pub fn optical(self) -> bool {
        self != Mission::Sentinel1
    }

    /// Whether its satellites carry several cameras worth naming.
    pub fn names_instrument(self) -> bool {
        matches!(self, Mission::Cbers | Mission::Daily)
    }
}

/// A file of a scene to download.
#[derive(Clone, Debug, PartialEq)]
pub struct Download {
    pub label: String,
    pub url: String,
}

/// One scene: a tile (Sentinel-2), a path/row (Landsat, CBERS) or a frame
/// (Sentinel-1).
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub id: String,
    pub mission: Mission,
    pub platform: String,
    /// The camera, where the platform has several.
    pub instrument: Option<String>,
    pub time: DateTime<Utc>,
    /// Cloud over the whole scene, in percent (optical scenes).
    pub cloud: Option<f64>,
    pub preview: Option<String>,
    pub tilejson: Option<String>,
    /// Its map tiles' address ({z}/{x}/{y}), where it needs no tilejson.
    pub tiles: Option<String>,
    /// A north-up picture of the scene filling its bbox, laid on the globe
    /// where there are no map tiles (the Data Cube's thumbnails, on a white
    /// ground).
    pub snapshot: Option<String>,
    /// The scene on its catalogue's map.
    pub page: Option<String>,
    pub downloads: Vec<Download>,
    /// Footprint rings, (lat, lon).
    pub rings: Vec<Vec<(f64, f64)>>,
    /// West, south, east, north.
    pub bbox: [f64; 4],
    /// Whether the footprint holds the area's centre.
    pub over_center: bool,
}

/// One overflight: the scenes taken together by one camera.
#[derive(Clone, Debug, PartialEq)]
pub struct Pass {
    pub mission: Mission,
    pub platform: String,
    pub instrument: Option<String>,
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
        format!("{}/{}@{}", self.platform, self.instrument.as_deref().unwrap_or(""), self.time.timestamp())
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

    /// Its files: the main scene's first, then the others'.
    pub fn downloads(&self) -> Vec<Download> {
        let main = self.main();
        let mut list = main.downloads.clone();
        for s in self.scenes.iter().filter(|s| s.id != main.id) {
            list.extend(s.downloads.iter().map(|d| Download { label: format!("{} ({})", d.label, short_id(&s.id)), url: d.url.clone() }));
        }
        list
    }
}

/// The distinctive end of a long scene id (its tile or path/row).
fn short_id(id: &str) -> String {
    id.split('_').rev().take(3).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("_")
}

/// Whether a catalogue's platform ("sentinel-2c", "landsat-9") is the
/// satellite named `name` ("Sentinel-2C", "Landsat 9").
pub fn same_platform(platform: &str, name: &str) -> bool {
    let norm = |s: &str| s.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect::<String>();
    norm(platform) == norm(name)
}

/// The search for one mission's scenes over the area, latest first. A GET
/// with no headers of its own: browsers send it without asking the server
/// first (no CORS preflight), which some of them refuse to do.
pub fn url(catalogue: Catalogue, mission: Mission, obs: &Observer, start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    let at = |t: DateTime<Utc>| t.to_rfc3339_opts(SecondsFormat::Secs, true);
    format!(
        "{}?collections={}&intersects={}&datetime={}/{}&limit={LIMIT}{}",
        catalogue.search(),
        catalogue.collections(mission),
        escape(&geometry(obs).to_string()),
        at(start),
        at(end),
        catalogue.sort()
    )
}

/// Percent-encodes all but the characters URLs keep as they are.
fn escape(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => char::from(b).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn geometry(obs: &Observer) -> Value {
    let round = |x: f64| (x * 1e5).round() / 1e5;
    let Some(area) = &obs.area else { return json!({ "type": "Point", "coordinates": [round(obs.lon), round(obs.lat)] }) };
    let polygons: Vec<Value> = area
        .rings
        .iter()
        .filter(|r| r.len() >= 3)
        .map(|ring| {
            let step = ring.len().div_ceil(MAX_VERTICES);
            let mut coords: Vec<[f64; 2]> = ring.iter().step_by(step).map(|&(lat, lon)| [round(lon), round(lat)]).collect();
            if coords.first() != coords.last() {
                coords.push(coords[0]);
            }
            json!([coords])
        })
        .collect();
    json!({ "type": "MultiPolygon", "coordinates": polygons })
}

/// West, south, east, north of the area (around a point, a small square).
fn area_box(obs: &Observer) -> [f64; 4] {
    match &obs.area {
        Some(area) => area.rings.iter().flatten().fold([180.0_f64, 90.0, -180.0, -90.0], |b, &(lat, lon)| {
            [b[0].min(lon), b[1].min(lat), b[2].max(lon), b[3].max(lat)]
        }),
        None => {
            let dlon = POINT_HALF_DEG / obs.lat.to_radians().cos().max(0.1);
            [obs.lon - dlon, obs.lat - POINT_HALF_DEG, obs.lon + dlon, obs.lat + POINT_HALF_DEG]
        }
    }
}

/// The scenes of a search's answer.
pub fn parse(text: &str, mission: Mission, obs: &Observer) -> Result<Vec<Scene>, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("catalogue: {e}"))?;
    let features = v["features"].as_array().ok_or("catalogue: no scenes in the answer")?;
    Ok(features.iter().filter_map(|f| scene(f, mission, obs)).collect())
}

fn scene(f: &Value, mission: Mission, obs: &Observer) -> Option<Scene> {
    let p = &f["properties"];
    let id = f["id"].as_str()?.to_string();
    let time = DateTime::parse_from_rfc3339(p["datetime"].as_str()?).ok()?.with_timezone(&Utc);
    let asset = |k: &str| f["assets"][k]["href"].as_str().map(str::to_string);
    let rings = rings(&f["geometry"]);
    let bbox = match f["bbox"].as_array().map(|b| b.iter().filter_map(Value::as_f64).collect::<Vec<_>>()) {
        Some(b) if b.len() == 4 => [b[0], b[1], b[2], b[3]],
        _ => rings.iter().flatten().fold([180.0_f64, 90.0, -180.0, -90.0], |b, &(lat, lon)| {
            [b[0].min(lon), b[1].min(lat), b[2].max(lon), b[3].max(lat)]
        }),
    };
    let (platform, instrument) = match p["platform"].as_str() {
        Some(platform) => (platform.to_string(), p["instruments"][0].as_str().map(str::to_ascii_uppercase)),
        // The Data Cube names them in the id only: CBERS_4A_WFI_…
        None => platform_from_id(&id).unwrap_or_else(|| (mission.label().to_string(), None)),
    };
    let rendered = asset("rendered_preview");
    Some(Scene {
        mission,
        platform,
        instrument,
        time,
        cloud: p["eo:cloud_cover"].as_f64(),
        // The Planetary Computer renders previews; the others link their
        // files' own (when they are on the open web).
        preview: rendered
            .as_ref()
            .map(|u| format!("{u}&max_size={PREVIEW_PX}"))
            .or_else(|| asset("thumbnail").filter(|u| u.starts_with("https://"))),
        tilejson: asset("tilejson"),
        tiles: None,
        snapshot: matches!(mission, Mission::Cbers | Mission::Amazonia).then(|| asset("thumbnail")).flatten(),
        page: f["links"]
            .as_array()
            .and_then(|links| links.iter().find(|l| l["rel"] == "preview"))
            .and_then(|l| l["href"].as_str())
            .map(str::to_string),
        downloads: downloads(f, rendered.as_deref(), obs),
        over_center: aoi::in_rings(&rings, obs.lat, obs.lon),
        id,
        rings,
        bbox,
    })
}

/// "CBERS_4A_WFI_20261015_208_132_L4" → ("CBERS-4A", "WFI").
fn platform_from_id(id: &str) -> Option<(String, Option<String>)> {
    const CAMERAS: [&str; 6] = ["WFI", "AWFI", "MUX", "WPM", "PAN5M", "PAN10M"];
    let parts: Vec<&str> = id.split('_').collect();
    let at = parts.iter().position(|p| CAMERAS.contains(p))?;
    let platform = parts[..at].join("-");
    // "AMAZONIA-1" reads as the satellite's name.
    let platform = match platform.as_str() {
        "AMAZONIA-1" => "Amazonia-1".to_string(),
        _ => platform,
    };
    Some((platform, Some(parts[at].to_string())))
}

/// What can be downloaded of a scene: a cut-out of the area where the
/// catalogue renders one, else its open files.
fn downloads(f: &Value, rendered: Option<&str>, obs: &Observer) -> Vec<Download> {
    let mut list = Vec::new();
    // The Planetary Computer cuts any box out of the scene, rendered as its
    // preview is (true colour, radar backscatter).
    if let Some(query) = rendered.and_then(|u| u.split_once('?')).map(|(_, q)| q) {
        let query: Vec<&str> = query.split('&').filter(|kv| !kv.starts_with("format=")).collect();
        let [w, s, e, n] = area_box(obs);
        list.push(Download {
            label: "Your area, as shown (GeoTIFF)".into(),
            url: format!("{PC_DATA}/item/bbox/{w:.5},{s:.5},{e:.5},{n:.5}.tif?{}&max_size={CUT_PX}", query.join("&")),
        });
        return list;
    }
    let Some(assets) = f["assets"].as_object() else { return list };
    // Open GeoTIFFs: Earth Search's true colour, the Data Cube's bands.
    for (key, a) in assets {
        let (Some(href), Some(kind)) = (a["href"].as_str(), a["type"].as_str()) else { continue };
        if !href.starts_with("https://") || !kind.starts_with("image/tiff") {
            continue;
        }
        let label = match key.as_str() {
            "visual" => "Whole scene, true colour (GeoTIFF)".to_string(),
            k if k.starts_with("BAND") => format!("Band {} (GeoTIFF)", &k[4..]),
            _ => continue,
        };
        list.push(Download { label, url: href.to_string() });
    }
    list
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

/// Scenes gathered by overflight and camera, latest first.
pub fn passes(mut scenes: Vec<Scene>) -> Vec<Pass> {
    scenes.sort_by_key(|s| std::cmp::Reverse(s.time));
    let mut out: Vec<Pass> = Vec::new();
    for s in scenes {
        let same = out.iter_mut().find(|p| {
            p.platform.eq_ignore_ascii_case(&s.platform)
                && p.instrument == s.instrument
                && (p.time - s.time).num_seconds().abs() <= SAME_PASS_S
        });
        match same {
            Some(p) => p.scenes.push(s),
            None => out.push(Pass {
                mission: s.mission,
                platform: s.platform.clone(),
                instrument: s.instrument.clone(),
                time: s.time,
                scenes: vec![s],
            }),
        }
    }
    out
}

/// NASA's daily true-colour views: satellite, camera, GIBS layer, local
/// solar time of the daytime pass, and pixel size in degrees.
pub const DAILY: [(&str, &str, &str, f64, f64); 5] = [
    ("Terra", "MODIS", "MODIS_Terra_CorrectedReflectance_TrueColor", 10.5, 0.00225),
    ("Aqua", "MODIS", "MODIS_Aqua_CorrectedReflectance_TrueColor", 13.5, 0.00225),
    ("Suomi NPP", "VIIRS", "VIIRS_SNPP_CorrectedReflectance_TrueColor", 13.5, 0.0034),
    ("NOAA-20", "VIIRS", "VIIRS_NOAA20_CorrectedReflectance_TrueColor", 12.7, 0.0034),
    ("NOAA-21", "VIIRS", "VIIRS_NOAA21_CorrectedReflectance_TrueColor", 13.1, 0.0034),
];
/// Days of daily views listed, back from yesterday (today's are not whole
/// before the evening).
pub const DAILY_DAYS: i64 = 14;
/// Half the side of the region a daily view shows around the area, at
/// least, in degrees of latitude: its pixels are hundreds of metres.
const REGION_HALF_DEG: f64 = 0.8;
const GIBS_TILES: &str = "https://gibs.earthdata.nasa.gov/wmts/epsg3857/best";
const SNAPSHOT: &str = "https://wvs.earthdata.nasa.gov/api/v1/snapshot";

/// The daily views of the region around the area, for the days before
/// `now` (UTC dates, as GIBS names them).
pub fn daily(obs: &Observer, now: DateTime<Utc>) -> Vec<Scene> {
    let area = area_box(obs);
    let region = region_box(area);
    let [w, s, e, n] = region;
    let ring = vec![(s, w), (s, e), (n, e), (n, w), (s, w)];
    (1..=DAILY_DAYS)
        .flat_map(|back| {
            let date = (now - Duration::days(back)).date_naive();
            let ring = &ring;
            DAILY.iter().map(move |&(platform, camera, layer, solar_h, deg)| {
                let day = date.format("%Y-%m-%d").to_string();
                // The pass over the area is near that local solar time.
                let utc_h = (solar_h - obs.lon / 15.0).rem_euclid(24.0);
                let time = date.and_time(NaiveTime::MIN).and_utc() + Duration::seconds((utc_h * 3600.0) as i64);
                let snapshot = |bbox: [f64; 4], px: u32, format: &str| {
                    let [w, s, e, n] = bbox;
                    let height = ((f64::from(px) * (n - s) / (e - w)).round() as u32).max(1);
                    format!(
                        "{SNAPSHOT}?REQUEST=GetSnapshot&TIME={day}&BBOX={s:.5},{w:.5},{n:.5},{e:.5}&CRS=EPSG:4326&LAYERS={layer}&FORMAT={format}&WIDTH={px}&HEIGHT={height}"
                    )
                };
                // Native pixels across, within what a snapshot gives.
                let native = |bbox: [f64; 4]| (((bbox[2] - bbox[0]) / deg).ceil() as u32).clamp(64, CUT_PX);
                Scene {
                    id: format!("{layer}/{day}"),
                    mission: Mission::Daily,
                    platform: platform.to_string(),
                    instrument: Some(camera.to_string()),
                    time,
                    cloud: None,
                    preview: Some(snapshot(region, PREVIEW_PX, "image/jpeg")),
                    tilejson: None,
                    tiles: Some(daily_tiles(layer, date)),
                    snapshot: None,
                    page: Some(format!(
                        "https://worldview.earthdata.nasa.gov/?v={w:.4},{s:.4},{e:.4},{n:.4}&l=Coastlines_15m,{layer}&t={day}-T12:00:00Z"
                    )),
                    downloads: vec![
                        Download { label: "Your area (GeoTIFF)".into(), url: snapshot(area, native(area), "image/tiff") },
                        Download { label: "The region shown (GeoTIFF)".into(), url: snapshot(region, native(region), "image/tiff") },
                    ],
                    rings: vec![ring.clone()],
                    bbox: region,
                    over_center: true,
                }
            })
        })
        .collect()
}

/// The map tiles of a GIBS layer on a day (UTC date), as Cesium takes them.
pub fn daily_tiles(layer: &str, date: NaiveDate) -> String {
    format!("{GIBS_TILES}/{layer}/default/{}/GoogleMapsCompatible_Level9/{{z}}/{{y}}/{{x}}.jpg", date.format("%Y-%m-%d"))
}

/// The area's box grown to a region a daily view is worth looking at.
fn region_box([w, s, e, n]: [f64; 4]) -> [f64; 4] {
    let (lat, lon) = ((s + n) / 2.0, (w + e) / 2.0);
    let half_lat = ((n - s) / 2.0 * 1.3).max(REGION_HALF_DEG);
    let half_lon = ((e - w) / 2.0 * 1.3).max(REGION_HALF_DEG / lat.to_radians().cos().max(0.1));
    [lon - half_lon, (lat - half_lat).max(-85.0), lon + half_lon, (lat + half_lat).min(85.0)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_cube_ids_name_platform_and_camera() {
        assert_eq!(platform_from_id("CBERS_4A_WFI_20261015_208_132_L4"), Some(("CBERS-4A".into(), Some("WFI".into()))));
        assert_eq!(platform_from_id("CBERS_4_MUX_20261027_158_118_L4"), Some(("CBERS-4".into(), Some("MUX".into()))));
        assert_eq!(platform_from_id("CBERS_4_AWFI_20261024_159_117_L4"), Some(("CBERS-4".into(), Some("AWFI".into()))));
        assert_eq!(platform_from_id("AMAZONIA_1_WFI_20261020_034_018_L4"), Some(("Amazonia-1".into(), Some("WFI".into()))));
        assert!(same_platform("CBERS-4A", "CBERS-4A") && same_platform("Amazonia-1", "Amazonia 1"));
    }

    #[test]
    fn daily_views_are_dated_before_today_near_their_pass() {
        let obs = Observer::new("Brasília", -15.79, -47.88, 1100.0);
        let now = DateTime::parse_from_rfc3339("2026-10-29T18:00:00Z").unwrap().with_timezone(&Utc);
        let views = daily(&obs, now);
        assert_eq!(views.len(), DAILY.len() * DAILY_DAYS as usize);
        let terra = &views[0];
        assert!(terra.id.ends_with("/2026-10-28"));
        // 10:30 local solar time at 47.88° W is about 13:42 UTC.
        assert_eq!(terra.time.format("%Y-%m-%d %H:%M").to_string(), "2026-10-28 13:41");
        assert!(terra.tiles.as_deref().unwrap().contains("/2026-10-28/GoogleMapsCompatible_Level9/{z}/{y}/{x}.jpg"));
        assert!(terra.preview.as_deref().unwrap().contains("TIME=2026-10-28&BBOX=-16.59000,"));
        let [w, s, e, n] = terra.bbox;
        assert!(w < -47.88 && e > -47.88 && s < -15.79 && n > -15.79 && n - s >= 1.6);
    }
}
