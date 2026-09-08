import { LEFT, RIGHT, TOP, BOTTOM } from "../../../native/webamp/packages/webamp-modern/src/skin/Cursor";

// Only the main container owns the host window. Auxiliary skin containers
// retain their own movement and geometry inside the renderer.
export function installHostWindow(root: any, send: (name: string, value?: unknown) => unknown) {
  const main = root.findContainer("main");
  if (!main) return;
  // Multi-window skins still need a desktop surface until each container has
  // its own native window; shrinking that surface would clip their playlist/EQ.
  const desktopSurface = root.getContainers().some((container: any) => container !== main && container.getVisible());
  let active: any = null;
  let lastGeometry = "";
  const publish = () => {
    if (desktopSurface) return;
    const layout = main.getcurlayout();
    if (!layout) return;
    if (active !== layout) {
      active = layout;
      main.setLocation(0, 0);
    }
    const data = {
      width: layout.getwidth(), height: layout.getheight(),
      minimumWidth: Math.max(1, layout._minimumWidth || 1),
      minimumHeight: Math.max(1, layout._minimumHeight || 1),
      maximumWidth: layout._maximumWidth || 16384,
      maximumHeight: layout._maximumHeight || 16384,
    };
    const key = JSON.stringify(data);
    if (key !== lastGeometry) { lastGeometry = key; send("windowGeometry", data); }
  };
  const resizeFromHost = () => {
    if (desktopSurface) return;
    const layout = main.getcurlayout();
    if (!layout) return;
    const width = document.documentElement.clientWidth;
    const height = document.documentElement.clientHeight;
    if (layout.getwidth() === width && layout.getheight() === height) return;
    layout.resize(0, 0, width, height);
    layout._invalidateSize();
    publish();
  };
  for (const layout of main._layouts) {
    layout.setMoving = (command: string, dx: number, dy: number) => {
      if (command === "start") send("windowMove");
      // Native movement owns the gesture, never translate the skin in its viewport.
    };
    // Preserve the desktop canvas for auxiliary windows, but never disable
    // dragging the host by its main skin title bar.
    if (desktopSurface) continue;
    const resizing = layout.setResizing.bind(layout);
    layout.setResizing = (command: string, dx: number, dy: number) => {
      if (command === "constraint") return resizing(command, dx, dy);
      if (command !== "start") return;
      const mask = layout._canResize;
      send("windowResize", { left: !!(mask & LEFT), right: !!(mask & RIGHT),
        top: !!(mask & TOP), bottom: !!(mask & BOTTOM) });
    };
  }
  const dispatch = root.dispatch.bind(root);
  root.dispatch = (action: string, param: unknown, target: unknown) => {
    if (action.toLowerCase() === "minimize") return send("windowMinimize");
    if (action.toLowerCase() === "close" && !target) {
      const focused = document.activeElement?.closest("container");
      if (focused === main.getDiv()) return send("restore");
    }
    return dispatch(action, param, target);
  };
  window.addEventListener("resize", resizeFromHost);
  const observer = new ResizeObserver(publish);
  for (const layout of main._layouts) observer.observe(layout.getDiv());
  publish();
  // Recover a geometry update dropped by the host's startup request budget.
  const refresh = window.setInterval(() => { lastGeometry = ""; publish(); }, 1000);
  window.addEventListener("pagehide", () => {
    window.clearInterval(refresh);
    window.removeEventListener("resize", resizeFromHost);
    observer.disconnect();
  }, { once: true });
}
