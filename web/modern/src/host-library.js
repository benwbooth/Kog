const LIBRARY_GUID = "{6b0edf80-c9a5-11d3-9f26-00c04f39ffc6}";

export function isLibraryComponent(value) {
  const guid = String(value || "").toLowerCase().replace(/^guid:/, "");
  return guid === LIBRARY_GUID || guid === "ml";
}

export function intersectRect(rect, clip) {
  const x = Math.max(rect.x, clip.x);
  const y = Math.max(rect.y, clip.y);
  const right = Math.min(rect.x + rect.width, clip.x + clip.width);
  const bottom = Math.min(rect.y + rect.height, clip.y + clip.height);
  return right > x && bottom > y ? { x, y, width: right - x, height: bottom - y } : null;
}

function visibleRect(element, document, viewport) {
  let rect = intersectRect(element.getBoundingClientRect(), viewport);
  for (let ancestor = element; rect && ancestor; ancestor = ancestor.parentElement) {
    const style = document.defaultView.getComputedStyle(ancestor);
    if (style.display === "none" || style.visibility !== "visible" || Number(style.opacity) === 0) return null;
    const bounds = ancestor.getBoundingClientRect();
    const clipX = ["hidden", "clip", "scroll", "auto"].includes(style.overflowX);
    const clipY = ["hidden", "clip", "scroll", "auto"].includes(style.overflowY);
    if (clipX || clipY) rect = intersectRect(rect, {
      x: clipX ? bounds.x : rect.x,
      y: clipY ? bounds.y : rect.y,
      width: clipX ? bounds.width : rect.width,
      height: clipY ? bounds.height : rect.height,
    });
  }
  return rect;
}

// Only geometry and bounded palette colors leave the renderer. The file browser is a trusted QML
// overlay; no filenames, model objects or filesystem commands enter this page.
export function libraryViewport(document, viewport) {
  // Menus retain hidden popup DOM between uses. Only an open, visible popup
  // should suppress the native overlay so web menu choices stay accessible.
  for (const popup of document.querySelectorAll(".popup-menu-container")) {
    if (visibleRect(popup, document, viewport)) return null;
  }
  let largest = null;
  for (const element of document.querySelectorAll("[data-kog-library]")) {
    const rect = visibleRect(element, document, viewport);
    if (!rect) continue;
    const top = document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2);
    if (top !== element && !element.contains(top)) continue;
    if (!largest || rect.width * rect.height > largest.width * largest.height) largest = rect;
  }
  if (!largest) return null;
  // Round inward so native pixel alignment cannot cover an adjacent skin control.
  const x = Math.ceil(largest.x), y = Math.ceil(largest.y);
  const width = Math.floor(largest.x + largest.width) - x;
  const height = Math.floor(largest.y + largest.height) - y;
  return width > 0 && height > 0 ? { x, y, width, height } : null;
}

export function publishLibraryViewport(send, document = globalThis.document, window = globalThis.window) {
  let previous = "";
  let previousStyle = "";
  let refreshTicks = 0;
  const update = () => {
    const palette = libraryStyle(document);
    const styleKey = JSON.stringify(palette);
    if (styleKey !== previousStyle || refreshTicks <= 1) { send("libraryStyle", palette); previousStyle = styleKey; }
    const rect = libraryViewport(document, { x: 0, y: 0, width: window.innerWidth, height: window.innerHeight });
    const next = JSON.stringify(rect);
    // WebChannel requests can be rate-limited during script startup. Refresh
    // even unchanged geometry once a second so a dropped update cannot leave
    // the native overlay permanently hidden or at stale coordinates.
    if (next !== previous || --refreshTicks <= 0) {
      send("libraryViewport", rect);
      previous = next;
      refreshTicks = 10;
    }
  };
  update();
  const timer = window.setInterval(update, 100);
  window.addEventListener("pagehide", () => window.clearInterval(timer), { once: true });
}

// Only bounded color values cross this bridge, never CSS, assets, or paths.
export function libraryStyle(document) {
  const root = document.getElementById?.("ui-root");
  const style = root ? document.defaultView.getComputedStyle(root) : null;
  const color = (name, fallback) => {
    const value = style?.getPropertyValue(`--color-${name}`).trim() || "";
    if (/^#[0-9a-f]{6}$/i.test(value)) return value;
    const rgb = value.match(/^rgb\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)$/i);
    return rgb && rgb.slice(1).every(n => Number(n) <= 255)
      ? "#" + rgb.slice(1).map(n => Number(n).toString(16).padStart(2, "0")).join("") : fallback;
  };
  return {
    background: color("wasabi-list-background", "#202020"),
    text: color("wasabi-list-text", "#ffffff"),
    selection: color("wasabi-list-text-selected-background", "#405880"),
    selectionText: color("wasabi-list-text-selected", "#ffffff"),
    frame: color("wasabi-window-background", "#808080"),
    header: color("wasabi-list-column-background", "#808080"),
    headerText: color("wasabi-list-column-text", "#000000"),
  };
}
