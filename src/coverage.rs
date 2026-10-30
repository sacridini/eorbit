//! The coverage map: how many scenes a mission, or one of its satellites,
//! acquired in the last months, by H3 cell. Element 84's Earth Search
//! counts its scenes for us (its STAC aggregations), by the cell that holds
//! each scene's centre. INPE's Brazil Data Cube cannot: the page lists its
//! scenes and counts, in each cell, those that cover the cell's centre.

use crate::archive;
use chrono::{DateTime, Months, SecondsFormat, Timelike, Utc};
use h3o::{CellIndex, LatLng, Resolution};
use serde_json::{Value, json};
use std::collections::HashMap;

const AGGREGATE: &str = "https://earth-search.aws.element84.com/v1/aggregate";
const DATA_CUBE: &str = "https://data.inpe.br/bdc/stac/v1/search";
/// The cells' H3 resolution. Each holds several scenes' worth of ground
/// (about seven Sentinel-2 tiles, two or three Landsat scenes), so that its
/// count follows the coverage rather than how many centres happen to fall
/// in it; and the world has 5,882 of them, fewer than the 10,000 an
/// aggregation returns.
const RESOLUTION: Resolution = Resolution::Two;
/// A cell's mean area, in km².
pub const CELL_KM2: u64 = 86_700;
/// Scenes per page of the Data Cube's search, its most.
pub const PAGE: usize = 1000;
/// The most pages of the Data Cube asked for.
pub const MAX_PAGES: usize = 40;
/// Spacing of the points tried in a footprint's box to find the cells it
/// may cover, in degrees: well under a cell's inner radius (about 1.4°),
/// so that every cell whose centre lies in the box holds one.
const PROBE_DEG: f64 = 0.5;
/// The spans offered, in months back from today.
pub const MONTHS: [u32; 3] = [1, 2, 3];
/// The colour of each class of count, from few scenes (pale) to many
/// (deep): one hue, light to dark.
pub const RAMP: [&str; 6] = ["#cde2fb", "#86b6ef", "#5598e7", "#2a78d6", "#1c5cab", "#104281"];

/// Where the scenes are counted.
pub enum Origin {
    EarthSearch { collection: &'static str, platform: Option<&'static str> },
    /// Collections, comma separated.
    DataCube(&'static str),
}

/// What is counted: a mission's collection, or one satellite of it.
pub struct Source {
    /// The group it is listed in.
    pub mission: &'static str,
    /// "All" for the whole mission.
    pub label: &'static str,
    pub origin: Origin,
}

impl Source {
    /// "Sentinel-2", "Sentinel-2A".
    pub fn name(&self) -> &'static str {
        if self.label == "All" { self.mission } else { self.label }
    }
}

const fn earth_search(mission: &'static str, label: &'static str, collection: &'static str, platform: Option<&'static str>) -> Source {
    Source { mission, label, origin: Origin::EarthSearch { collection, platform } }
}

const fn data_cube(label: &'static str, collections: &'static str) -> Source {
    Source { mission: "CBERS and Amazonia", label, origin: Origin::DataCube(collections) }
}

pub const SOURCES: [Source; 14] = [
    earth_search("Sentinel-2", "All", "sentinel-2-l2a", None),
    earth_search("Sentinel-2", "Sentinel-2A", "sentinel-2-l2a", Some("sentinel-2a")),
    earth_search("Sentinel-2", "Sentinel-2B", "sentinel-2-l2a", Some("sentinel-2b")),
    earth_search("Sentinel-2", "Sentinel-2C", "sentinel-2-l2a", Some("sentinel-2c")),
    earth_search("Landsat", "All", "landsat-c2-l2", None),
    earth_search("Landsat", "Landsat 8", "landsat-c2-l2", Some("landsat-8")),
    earth_search("Landsat", "Landsat 9", "landsat-c2-l2", Some("landsat-9")),
    earth_search("Sentinel-1", "All", "sentinel-1-grd", None),
    earth_search("Sentinel-1", "Sentinel-1C", "sentinel-1-grd", Some("sentinel-1c")),
    earth_search("Sentinel-1", "Sentinel-1D", "sentinel-1-grd", Some("sentinel-1d")),
    data_cube("CBERS-4 and 4A", "CB4-MUX-L4-DN-1,CB4-WFI-L4-DN-1,CB4A-MUX-L4-DN-1,CB4A-WFI-L4-DN-1,CB4A-WPM-L4-DN-1"),
    data_cube("CBERS-4", "CB4-MUX-L4-DN-1,CB4-WFI-L4-DN-1"),
    data_cube("CBERS-4A", "CB4A-MUX-L4-DN-1,CB4A-WFI-L4-DN-1,CB4A-WPM-L4-DN-1"),
    data_cube("Amazonia-1", "AMZ1-WFI-L4-DN-1"),
];

/// The groups of SOURCES, in order.
pub const MISSIONS: [&str; 4] = ["Sentinel-2", "Landsat", "Sentinel-1", "CBERS and Amazonia"];

/// The map asked for: whose scenes, over how many months.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ask {
    /// Which of SOURCES.
    pub source: usize,
    pub months: u32,
}

impl Ask {
    pub const DEFAULT: Ask = Ask { source: 0, months: 1 };

    pub fn source(self) -> &'static Source {
        &SOURCES[self.source.min(SOURCES.len() - 1)]
    }

    /// Whether Earth Search counts the scenes (else the page does).
    pub fn aggregated(self) -> bool {
        matches!(self.source().origin, Origin::EarthSearch { .. })
    }

    /// The span counted, ending at the start of this hour (so that the
    /// same map is not counted again within the hour).
    pub fn span(self, now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
        let end = now.with_minute(0).and_then(|t| t.with_second(0)).and_then(|t| t.with_nanosecond(0)).unwrap_or(now);
        (end.checked_sub_months(Months::new(self.months)).unwrap_or(end), end)
    }

    /// "month", "2 months".
    pub fn span_label(self) -> String {
        if self.months == 1 { "month".into() } else { format!("{} months", self.months) }
    }

    /// Which scenes a cell's count holds, after "N scenes of …".
    pub fn counted(self) -> &'static str {
        if self.aggregated() { "centred in this cell" } else { "over this cell's centre" }
    }

    /// The search: Earth Search's counts, or a page (from 1) of the Data
    /// Cube's scenes, with only their footprints. A GET with no headers of
    /// its own (no CORS preflight).
    pub fn url(self, now: DateTime<Utc>, page: usize) -> String {
        let (start, end) = self.span(now);
        let at = |t: DateTime<Utc>| t.to_rfc3339_opts(SecondsFormat::Secs, true);
        match self.source().origin {
            Origin::EarthSearch { collection, platform } => {
                let query = platform
                    .map(|p| format!("&query={}", archive::escape(&json!({ "platform": { "eq": p } }).to_string())))
                    .unwrap_or_default();
                format!(
                    "{AGGREGATE}?collections={collection}&datetime={}/{}{query}\
                     &aggregations=centroid_geohex_grid_frequency&centroid_geohex_grid_frequency_precision={}",
                    at(start),
                    at(end),
                    u8::from(RESOLUTION),
                )
            }
            Origin::DataCube(collections) => format!(
                "{DATA_CUBE}?collections={collections}&datetime={}/{}&limit={PAGE}&page={page}\
                 &fields=bbox,geometry,-properties,-assets,-links",
                at(start),
                at(end),
            ),
        }
    }
}

/// The scenes counted per cell.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Counts {
    pub cells: Vec<(CellIndex, u32)>,
    /// The scenes counted.
    pub scenes: u64,
}

impl Counts {
    /// Earth Search's counts.
    pub fn parse(text: &str) -> Result<Counts, String> {
        let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let buckets = v["aggregations"]
            .as_array()
            .and_then(|a| a.iter().find(|g| g["name"] == "centroid_geohex_grid_frequency"))
            .and_then(|g| g["buckets"].as_array())
            .ok_or("no counts in the answer")?;
        let cells: Vec<(CellIndex, u32)> = buckets
            .iter()
            .filter_map(|b| {
                let cell = b["key"].as_str()?.parse::<CellIndex>().ok()?;
                Some((cell, b["frequency"].as_u64()?.min(u64::from(u32::MAX)) as u32))
            })
            .collect();
        let scenes = cells.iter().map(|&(_, n)| u64::from(n)).sum();
        Ok(Counts { cells, scenes })
    }

    /// The scenes of the Data Cube's pages, each counted in the cells
    /// whose centre its footprint covers.
    pub fn footprints(pages: &[String]) -> Result<Counts, String> {
        let mut counts: HashMap<CellIndex, u32> = HashMap::new();
        let mut scenes = 0;
        for text in pages {
            let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
            let features = v["features"].as_array().ok_or("no scenes in the answer")?;
            for f in features {
                let rings = outer_rings(&f["geometry"]);
                if rings.is_empty() {
                    continue;
                }
                scenes += 1;
                for cell in covered(&rings) {
                    *counts.entry(cell).or_default() += 1;
                }
            }
        }
        let mut cells: Vec<(CellIndex, u32)> = counts.into_iter().collect();
        cells.sort_unstable();
        Ok(Counts { cells, scenes })
    }

    /// The lower bound of each class after the first: quantiles of the
    /// counts, rounded to two figures, so that each class holds about as
    /// many cells.
    pub fn breaks(&self) -> Vec<u32> {
        let mut counts: Vec<u32> = self.cells.iter().map(|&(_, n)| n).collect();
        counts.sort_unstable();
        let mut out: Vec<u32> = Vec::new();
        for i in 1..RAMP.len() {
            let Some(&q) = counts.get(counts.len() * i / RAMP.len()) else { break };
            let b = two_figures(q).max(2);
            if out.last().is_none_or(|&last| b > last) {
                out.push(b);
            }
        }
        out
    }

    /// For the globe, per cell: the number of its corners, the latitude
    /// and longitude of its centre, its count, its class, then its corners
    /// as longitude, latitude.
    pub fn geometry(&self, breaks: &[u32]) -> Vec<f64> {
        let mut out = Vec::with_capacity(self.cells.len() * 17);
        for &(cell, n) in &self.cells {
            let corners = cell.boundary();
            let centre = LatLng::from(cell);
            out.extend([corners.len() as f64, centre.lat(), centre.lng(), f64::from(n), class(n, breaks) as f64]);
            out.extend(corners.iter().flat_map(|c| [c.lng(), c.lat()]));
        }
        out
    }
}

/// How many scenes the Data Cube holds for a search, from its first page.
pub fn matched(text: &str) -> Option<usize> {
    let v: Value = serde_json::from_str(text).ok()?;
    v["numberMatched"].as_u64().or_else(|| v["context"]["matched"].as_u64()).map(|n| n as usize)
}

/// The outer rings of a (multi)polygon, as (lon, lat).
fn outer_rings(g: &Value) -> Vec<Vec<(f64, f64)>> {
    let ring = |r: &Value| -> Vec<(f64, f64)> {
        r.as_array().into_iter().flatten().filter_map(|p| Some((p[0].as_f64()?, p[1].as_f64()?))).collect()
    };
    let polygons: Vec<&Value> = match g["type"].as_str() {
        Some("Polygon") => vec![&g["coordinates"]],
        Some("MultiPolygon") => g["coordinates"].as_array().map(|a| a.iter().collect()).unwrap_or_default(),
        _ => Vec::new(),
    };
    polygons.into_iter().map(|p| ring(&p[0])).filter(|r| r.len() >= 3).collect()
}

/// The cells whose centre a footprint covers: those of points tried
/// across its box, kept when their centre lies in one of its rings.
fn covered(rings: &[Vec<(f64, f64)>]) -> Vec<CellIndex> {
    let mut found = Vec::new();
    for ring in rings {
        let (w, s, e, n) = ring.iter().fold((180.0_f64, 90.0_f64, -180.0_f64, -90.0_f64), |(w, s, e, n), &(x, y)| {
            (w.min(x), s.min(y), e.max(x), n.max(y))
        });
        // A ring across the antimeridian (none in the Data Cube) is skipped.
        if e - w > 180.0 {
            continue;
        }
        let steps = |a: f64, b: f64| ((b - a) / PROBE_DEG).ceil() as usize;
        for i in 0..=steps(s, n) {
            let lat = (s + i as f64 * PROBE_DEG).min(n);
            for j in 0..=steps(w, e) {
                let lon = (w + j as f64 * PROBE_DEG).min(e);
                if let Ok(p) = LatLng::new(lat, lon) {
                    found.push(p.to_cell(RESOLUTION));
                }
            }
        }
    }
    found.sort_unstable();
    found.dedup();
    found.retain(|&cell| {
        let c = LatLng::from(cell);
        rings.iter().any(|r| inside(r, c.lng(), c.lat()))
    });
    found
}

/// Whether (x, y) lies in a ring (even-odd rule).
fn inside(ring: &[(f64, f64)], x: f64, y: f64) -> bool {
    let mut odd = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let ((xi, yi), (xj, yj)) = (ring[i], ring[j]);
        if (yi > y) != (yj > y) && x < xi + (y - yi) * (xj - xi) / (yj - yi) {
            odd = !odd;
        }
        j = i;
    }
    odd
}

/// The class of a count, an index of RAMP.
pub fn class(n: u32, breaks: &[u32]) -> usize {
    breaks.iter().filter(|&&b| n >= b).count()
}

/// Rounded to two significant figures.
fn two_figures(n: u32) -> u32 {
    if n < 100 {
        return n;
    }
    let unit = 10_u32.pow(n.ilog10() - 1);
    (n + unit / 2) / unit * unit
}

/// The coverage map as the page has it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Coverage {
    /// What it counts.
    pub ask: Option<Ask>,
    pub loading: bool,
    pub counts: Counts,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_are_read_by_cell() {
        let text = json!({ "aggregations": [{ "name": "centroid_geohex_grid_frequency", "buckets": [
            { "key": "82f1a7fffffffff", "frequency": 899 },
            { "key": "not a cell", "frequency": 3 },
            { "key": "82ef0ffffffffff", "frequency": 7 },
        ]}]})
        .to_string();
        let counts = Counts::parse(&text).unwrap();
        assert_eq!(counts.cells.len(), 2);
        assert_eq!(counts.scenes, 906);
        assert!(Counts::parse("{}").is_err());
        // A hexagon: six corners after the five leading numbers.
        assert_eq!(counts.geometry(&counts.breaks()).len(), 2 * (5 + 12));
    }

    #[test]
    fn a_footprint_counts_in_the_cells_whose_centre_it_covers() {
        // Two scenes over central Brazil, one inside the other.
        let square = |w: f64, s: f64, side: f64| {
            json!({ "geometry": { "type": "Polygon", "coordinates": [[[w, s], [w + side, s], [w + side, s + side], [w, s + side], [w, s]]] } })
        };
        let page = json!({ "features": [square(-52.0, -16.0, 8.0), square(-50.0, -14.0, 4.0), { "geometry": null }] }).to_string();
        let counts = Counts::footprints(&[page]).unwrap();
        assert_eq!(counts.scenes, 2);
        // The big one covers some cells' centres (an 8° square, cells of
        // about 3°), the small one fewer, all of them also in the big one.
        let big = counts.cells.iter().filter(|&&(_, n)| n >= 1).count();
        let both = counts.cells.iter().filter(|&&(_, n)| n == 2).count();
        assert!(big >= 4, "{big}");
        assert!(both >= 1 && both < big, "{both} of {big}");
        for &(cell, _) in &counts.cells {
            let c = LatLng::from(cell);
            assert!((-52.0..=-44.0).contains(&c.lng()) && (-16.0..=-8.0).contains(&c.lat()));
        }
    }

    #[test]
    fn classes_split_the_counts() {
        let cell = CellIndex::try_from(0x830796fffffffff_u64).unwrap();
        let counts = Counts { cells: (1..=600).map(|n| (cell, n)).collect(), scenes: 0 };
        let breaks = counts.breaks();
        assert_eq!(breaks, vec![100, 200, 300, 400, 500]);
        assert_eq!(class(1, &breaks), 0);
        assert_eq!(class(250, &breaks), 2);
        assert_eq!(class(600, &breaks), RAMP.len() - 1);
        assert_eq!(two_figures(1234), 1200);
        assert_eq!(two_figures(87), 87);
    }

    #[test]
    fn searches_ask_for_a_satellite_or_a_page() {
        let now = Utc::now();
        let url = Ask { source: 2, months: 2 }.url(now, 1);
        assert!(url.contains("collections=sentinel-2-l2a"));
        assert!(url.contains("%22sentinel-2b%22"));
        assert!(url.ends_with("precision=2"));
        assert!(!Ask { source: 4, months: 1 }.url(now, 1).contains("query="));
        let amazonia = SOURCES.iter().position(|s| s.name() == "Amazonia-1").unwrap();
        let url = Ask { source: amazonia, months: 3 }.url(now, 3);
        assert!(url.starts_with(DATA_CUBE) && url.contains("AMZ1-WFI-L4-DN-1") && url.contains("&page=3"));
        assert_eq!(matched(r#"{"context": {"matched": 3443}}"#), Some(3443));
    }
}
