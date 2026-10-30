// The page's address (which carries the shared state) and the clipboard.

export function hash() {
  return location.hash.slice(1);
}

/// Replaces the state in the address, without a new history entry.
export function setHash(h) {
  if (location.hash.slice(1) === h) return;
  history.replaceState(null, "", h ? `#${h}` : location.pathname + location.search);
}

export function link() {
  return location.href;
}

/// Resolves to whether the text reached the clipboard.
export async function copy(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    // Without the clipboard API (or its permission): the old way.
    const area = document.createElement("textarea");
    area.value = text;
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.append(area);
    area.select();
    const ok = document.execCommand("copy");
    area.remove();
    return ok;
  }
}

/// Saves the file at a URL (one the browser cannot show downloads).
export function save(url) {
  const a = document.createElement("a");
  a.href = url;
  a.download = "";
  a.rel = "noopener";
  document.body.append(a);
  a.click();
  a.remove();
}

/// Calls back with the new state when the user edits the address.
export function onHashChange(callback) {
  addEventListener("hashchange", () => callback(location.hash.slice(1)));
}

/// Closes an open list of downloads when the user clicks elsewhere or
/// presses Escape, so that lists do not pile up over the pictures.
export function dismissMenus() {
  const close = (keep) => {
    for (const d of document.querySelectorAll("details.downloads[open]")) {
      if (!keep || !d.contains(keep)) d.open = false;
    }
  };
  addEventListener("click", (e) => close(e.target));
  addEventListener("keydown", (e) => {
    if (e.key === "Escape") close(null);
  });
  // One list open at a time.
  addEventListener(
    "toggle",
    (e) => {
      if (e.target.matches?.("details.downloads") && e.target.open) close(e.target);
    },
    true,
  );
}
