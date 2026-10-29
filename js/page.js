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

/// Calls back with the new state when the user edits the address.
export function onHashChange(callback) {
  addEventListener("hashchange", () => callback(location.hash.slice(1)));
}
