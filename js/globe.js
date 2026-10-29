// The Cesium globe: the fleet as points, the selected satellite's orbit,
// ground track and swath, and the area of interest. Rust computes every
// position; this module only draws. Setters keep their data so that the
// globe can be (re)drawn whenever the viewer is ready.

const C = () => window.Cesium;

const BASES = {
  imagery: () =>
    new (C().UrlTemplateImageryProvider)({
      url: "https://tiles.maps.eox.at/wmts/1.0.0/s2cloudless-2024_3857/default/GoogleMapsCompatible/{z}/{y}/{x}.jpg",
      maximumLevel: 14,
      credit: new (C().Credit)(
        'Sentinel-2 cloudless by <a href="https://s2maps.eu">EOX IT Services GmbH</a> (contains modified Copernicus Sentinel data 2024)',
        true,
      ),
    }),
  map: () =>
    new (C().UrlTemplateImageryProvider)({
      url: "https://services.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Dark_Gray_Base/MapServer/tile/{z}/{y}/{x}",
      maximumLevel: 16,
      credit: new (C().Credit)("Tiles © Esri — Esri, HERE, Garmin, © OpenStreetMap contributors", true),
    }),
};

let viewer = null;
let points = null;
let labels = null;
let fleet = [];
let selected = -1;
let base = "imagery";
let showLabels = true;
let picking = false;
let onSat = null;
let onGround = null;
let orbit = null;
let highlight = "[]";
let aoi = null;
let orbitEntities = [];
let highlightEntities = [];
let aoiEntities = [];
const scratch = { position: null, date: null };

export function init(container, credits, satCallback, groundCallback) {
  const Cesium = C();
  onSat = satCallback;
  onGround = groundCallback;
  viewer = new Cesium.Viewer(container, {
    baseLayer: false,
    animation: false,
    timeline: false,
    baseLayerPicker: false,
    geocoder: false,
    homeButton: false,
    sceneModePicker: false,
    navigationHelpButton: false,
    fullscreenButton: false,
    infoBox: false,
    selectionIndicator: false,
    shouldAnimate: false,
    creditContainer: document.getElementById(credits),
  });
  const scene = viewer.scene;
  scene.globe.baseColor = Cesium.Color.fromCssColorString("#0d2236");
  scene.globe.enableLighting = true;
  scene.globe.dynamicAtmosphereLighting = true;
  scene.globe.dynamicAtmosphereLightingFromSun = true;
  scene.globe.showGroundAtmosphere = true;
  scene.skyAtmosphere.show = true;
  scene.highDynamicRange = false;
  viewer.clock.shouldAnimate = false;
  scratch.position = new Cesium.Cartesian3();

  points = scene.primitives.add(new Cesium.PointPrimitiveCollection());
  labels = scene.primitives.add(new Cesium.LabelCollection());

  const handler = new Cesium.ScreenSpaceEventHandler(scene.canvas);
  handler.setInputAction((click) => {
    const hit = scene.pick(click.position);
    if (Cesium.defined(hit) && hit.id && hit.id.sat !== undefined) {
      onSat(hit.id.sat);
      return;
    }
    if (!picking) return;
    const at = viewer.camera.pickEllipsoid(click.position, scene.globe.ellipsoid);
    if (!at) return;
    const g = Cesium.Cartographic.fromCartesian(at);
    onGround(Cesium.Math.toDegrees(g.latitude), Cesium.Math.toDegrees(g.longitude));
  }, Cesium.ScreenSpaceEventType.LEFT_CLICK);
  handler.setInputAction((move) => {
    const hit = scene.pick(move.endPosition);
    const overSat = Cesium.defined(hit) && hit.id && hit.id.sat !== undefined;
    scene.canvas.style.cursor = overSat ? "pointer" : picking ? "crosshair" : "";
  }, Cesium.ScreenSpaceEventType.MOUSE_MOVE);

  setBase(base);
  drawFleet();
  if (orbit) drawOrbit();
  drawHighlight();
  drawAoi();
}

export function setBase(name) {
  base = name;
  if (!viewer) return;
  const layers = viewer.imageryLayers;
  layers.removeAll();
  const layer = layers.addImageryProvider(BASES[name]());
  // The night side keeps some of the map, so that it stays readable.
  layer.nightAlpha = name === "map" ? 1.0 : 0.55;
  layer.dayAlpha = 1.0;
}

/// [{short, name, color}] in the order of the positions given to update().
export function setFleet(json) {
  fleet = JSON.parse(json);
  drawFleet();
}

function drawFleet() {
  if (!viewer) return;
  const Cesium = C();
  points.removeAll();
  labels.removeAll();
  fleet.forEach((s, i) => {
    const color = Cesium.Color.fromCssColorString(s.color);
    points.add({
      id: { sat: i },
      pixelSize: 7,
      color,
      outlineColor: Cesium.Color.fromCssColorString("#06101c"),
      outlineWidth: 2,
      show: false,
    });
    labels.add({
      id: { sat: i },
      text: s.short,
      font: '600 12px "Archivo", sans-serif',
      fillColor: color,
      outlineColor: Cesium.Color.fromCssColorString("#06101c").withAlpha(0.9),
      outlineWidth: 3,
      style: Cesium.LabelStyle.FILL_AND_OUTLINE,
      pixelOffset: new Cesium.Cartesian2(9, -1),
      horizontalOrigin: Cesium.HorizontalOrigin.LEFT,
      verticalOrigin: Cesium.VerticalOrigin.CENTER,
      show: false,
    });
  });
  styleSelected();
}

function styleSelected() {
  if (!viewer) return;
  const Cesium = C();
  for (let i = 0; i < points.length; i++) {
    const p = points.get(i);
    const on = i === selected;
    p.pixelSize = on ? 12 : 7;
    p.outlineColor = on ? Cesium.Color.WHITE : Cesium.Color.fromCssColorString("#06101c");
    p.outlineWidth = on ? 2.5 : 2;
    labels.get(i).font = on ? '700 14px "Archivo", sans-serif' : '600 12px "Archivo", sans-serif';
  }
}

/// Sets the simulated time (ms since the epoch) and the fleet's positions,
/// as x, y, z in metres (Earth-fixed) per satellite; NaN hides one.
export function update(ms, xyz) {
  if (!viewer) return;
  const Cesium = C();
  scratch.date = Cesium.JulianDate.fromDate(new Date(ms), scratch.date ?? undefined);
  viewer.clock.currentTime = scratch.date;
  const n = Math.min(points.length, xyz.length / 3);
  for (let i = 0; i < n; i++) {
    const x = xyz[3 * i];
    const p = points.get(i);
    const l = labels.get(i);
    if (Number.isNaN(x)) {
      p.show = false;
      l.show = false;
      continue;
    }
    Cesium.Cartesian3.fromElements(x, xyz[3 * i + 1], xyz[3 * i + 2], scratch.position);
    p.position = scratch.position;
    l.position = scratch.position;
    p.show = true;
    l.show = showLabels || i === selected;
  }
}

export function setSelected(index) {
  selected = index;
  styleSelected();
}

/// The selected satellite's orbit (x, y, z in metres), its ground track
/// (lon, lat in degrees) and its swaths (JSON, see drawCorridors).
export function setOrbit(xyz, track, swaths, color) {
  orbit = { xyz: Array.from(xyz), track: Array.from(track), swaths: JSON.parse(swaths), color };
  drawOrbit();
}

export function clearOrbit() {
  orbit = null;
  clear(orbitEntities);
}

function clear(list) {
  if (viewer) list.forEach((e) => viewer.entities.remove(e));
  list.length = 0;
}

function drawOrbit() {
  clear(orbitEntities);
  if (!viewer || !orbit) return;
  const Cesium = C();
  const color = Cesium.Color.fromCssColorString(orbit.color);
  const positions = [];
  for (let i = 0; i + 2 < orbit.xyz.length; i += 3) {
    positions.push(new Cesium.Cartesian3(orbit.xyz[i], orbit.xyz[i + 1], orbit.xyz[i + 2]));
  }
  orbitEntities.push(
    viewer.entities.add({
      polyline: {
        positions,
        width: 6,
        arcType: Cesium.ArcType.NONE,
        material: new Cesium.PolylineGlowMaterialProperty({ glowPower: 0.18, color: color.withAlpha(0.85) }),
      },
    }),
  );
  if (orbit.track.length >= 4) {
    orbitEntities.push(
      viewer.entities.add({
        polyline: {
          positions: Cesium.Cartesian3.fromDegreesArray(orbit.track),
          width: 1.5,
          clampToGround: true,
          material: new Cesium.PolylineDashMaterialProperty({ color: color.withAlpha(0.75), dashLength: 14 }),
        },
      }),
    );
  }
  drawCorridors(orbit.swaths, orbitEntities, 0.16);
}

/// Swaths as [{pts: [lon, lat, ...], width: metres, color}].
function drawCorridors(swaths, list, alpha) {
  const Cesium = C();
  for (const s of swaths) {
    if (s.pts.length < 4) continue;
    list.push(
      viewer.entities.add({
        corridor: {
          positions: Cesium.Cartesian3.fromDegreesArray(s.pts),
          width: s.width,
          height: 0,
          material: Cesium.Color.fromCssColorString(s.color).withAlpha(alpha),
        },
      }),
    );
  }
}

/// The swath of the selected opportunity, drawn stronger.
export function setHighlight(swaths) {
  highlight = swaths;
  drawHighlight();
}

function drawHighlight() {
  clear(highlightEntities);
  if (!viewer) return;
  drawCorridors(JSON.parse(highlight), highlightEntities, 0.42);
}

/// The area of interest: a point, and its rings ([[lon, lat, ...], ...]).
export function setAoi(name, lat, lon, rings) {
  aoi = { name, lat, lon, rings: JSON.parse(rings) };
  drawAoi();
}

function drawAoi() {
  clear(aoiEntities);
  if (!viewer || !aoi) return;
  const Cesium = C();
  const amber = Cesium.Color.fromCssColorString("#ffb547");
  for (const ring of aoi.rings) {
    if (ring.length < 6) continue;
    const positions = Cesium.Cartesian3.fromDegreesArray(ring);
    aoiEntities.push(
      viewer.entities.add({
        polygon: { hierarchy: positions, material: amber.withAlpha(0.18), height: 0 },
      }),
      viewer.entities.add({
        polyline: { positions: positions.concat([positions[0]]), width: 2, clampToGround: true, material: amber },
      }),
    );
  }
  aoiEntities.push(
    viewer.entities.add({
      position: Cesium.Cartesian3.fromDegrees(aoi.lon, aoi.lat),
      point: {
        pixelSize: 9,
        color: amber,
        outlineColor: Cesium.Color.fromCssColorString("#06101c"),
        outlineWidth: 3,
        disableDepthTestDistance: Number.POSITIVE_INFINITY,
      },
      label: {
        text: aoi.name,
        font: '600 13px "Archivo", sans-serif',
        fillColor: amber,
        outlineColor: Cesium.Color.fromCssColorString("#06101c"),
        outlineWidth: 4,
        style: Cesium.LabelStyle.FILL_AND_OUTLINE,
        pixelOffset: new Cesium.Cartesian2(0, -18),
        verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
        disableDepthTestDistance: Number.POSITIVE_INFINITY,
      },
    }),
  );
}

export function flyTo(lat, lon, height, seconds) {
  if (!viewer) return;
  const Cesium = C();
  viewer.camera.flyTo({ destination: Cesium.Cartesian3.fromDegrees(lon, lat, height), duration: seconds });
}

export function setLabels(on) {
  showLabels = on;
}

export function setPicking(on) {
  picking = on;
  if (viewer) viewer.scene.canvas.style.cursor = on ? "crosshair" : "";
}
