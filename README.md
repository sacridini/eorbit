# EOrbit

When will Sentinel, Landsat, CBERS, MODIS, VIIRS and other Earth observation
missions image your area? EOrbit answers on a 3D globe, in the browser:
<https://sacridini.github.io/eorbit/>.

It is the graphical sibling of [orbitui](https://github.com/sacridini/orbitui),
the terminal planner, and uses its library for orbits, predictions, the
mission catalogue, clouds and place search.

## What it shows

- The fleet in real time on the globe, with the night side shaded; for the
  selected satellite its orbit, ground track and swath.
- The area of interest: search a place, type `lat, lon`, pick it on the globe,
  or load (or drop) a GeoJSON polygon.
- Upcoming imaging opportunities: off-nadir angle, share of the area imaged,
  forecast cloud cover. Opening one takes the clock to it, highlights its
  swath and plots the satellite's path across the area's sky.
- A quality score (0 to 100) for each opportunity: clear sky, Sun height for
  optical sensors, viewing angle and share of the area imaged. The list can be
  sorted by it, and the detail explains it, with a local map of the swath over
  the area.
- A strip of every opportunity in the forecast (3, 7, 14 or 30 days) against
  night and cloud cover under the globe; dragging along it moves the
  simulated clock. Escape or "Reset view" deselects the satellite and returns
  to the area.

Four more views share the screen with the opportunities:

- Timeline: zoomable from 6 hours to 14 days, with the Sun's height as a sky
  band, the forecast cloud cover in total and by layer (low, middle, high),
  and a row per satellite whose bars grow with the score.
- Calendar: days by hours, tinted by the sky over the area, each hour holding
  its best opportunity, with the best chance of the forecast on top.
- Revisit: how often each satellite (and the fleet, and the good images
  alone) images the area, its longest wait and the gaps on a strip.
- Images: the scenes Sentinel-2, Landsat, Sentinel-1, CBERS-4/4A and
  Amazonia-1 actually acquired over the area in the last 30 to 180 days, with
  their previews and cloud cover, the latest clear image and how often one
  comes. A scene opens on the globe, draped over the map, and downloads as
  GeoTIFF: the area cut out of the scene (Planetary Computer), the whole
  scene in true colour (Earth Search) or its bands (CBERS, Amazonia).

The page's address follows the screen (area, polygon included, view,
satellite, opportunity, forecast span), so "Share" copies a link that opens
the same thing elsewhere.

Opportunities export to a spreadsheet (CSV) or a calendar (ICS). ESA's
acquisition plans for Sentinel-1 and Sentinel-2 (KML, loaded or dropped on the
page) tell whether each pass is planned. The forecast runs in a Web Worker, so
the page stays responsive while it computes.

## Build

Requires Rust with the `wasm32-unknown-unknown` target and
[Trunk](https://trunkrs.dev):

```sh
trunk serve          # http://127.0.0.1:8080
trunk build --release
```

## Layout

- `src/model.rs`: state, downloads (Celestrak elements, Open-Meteo clouds and
  places) and the per-frame loop.
- `src/bin/worker.rs`, `src/wire.rs`: the forecast's Web Worker and its
  messages.
- `src/quality.rs`: the score; `src/sky.rs`: the cloud layers;
  `src/archive.rs`: the acquired scenes; `src/share.rs`: the shared links.
- `src/ui/`: the panels and views ([Leptos](https://leptos.dev)).
- `src/globe.rs`: positions, orbits and swaths computed in Rust for the globe.
- `js/globe.js`: draws them on [CesiumJS](https://cesium.com/platform/cesiumjs/).

## Data

- Orbital elements: Celestrak GP data (OMM), with orbitui's bundled snapshot.
- Acquired scenes, their previews and map tiles: the
  [Microsoft Planetary Computer](https://planetarycomputer.microsoft.com)
  STAC catalogue (Sentinel-2 L2A, Landsat Collection 2 Level-2, Sentinel-1
  GRD); [Earth Search](https://element84.com/earth-search/) by Element 84 on
  AWS when the browser cannot reach it (privacy extensions often block it);
  INPE's [Brazil Data Cube](https://data.inpe.br/bdc/) for CBERS-4, CBERS-4A
  and Amazonia-1 (Level 4).
- Cloud forecast and place names: [Open-Meteo.com](https://open-meteo.com)
  (CC BY 4.0), with [GeoNames](https://www.geonames.org) for places.
- Base maps: [Sentinel-2 cloudless](https://s2maps.eu) by EOX IT Services GmbH
  (CC BY-NC-SA 4.0, contains modified Copernicus Sentinel data 2024) and
  Esri's World Dark Gray Base.

## License

MIT
