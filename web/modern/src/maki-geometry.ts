import GuiObj from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/GuiObj";
import SystemObject from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/SystemObject";
import Layout from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Layout";

// MAKI dimensions are client dimensions, not the XML relative offsets.
const resolving = { w: new Set<any>(), h: new Set<any>() };
function dimension(object: any, axis: "w" | "h"): number {
  const seen = resolving[axis];
  if (seen.has(object)) throw new Error("Cyclic MAKI geometry parent");
  seen.add(object);
  try {
    let value = Number(object[`_${axis}`]) || 0;
    if (String(object[`_relat${axis}`]) === "1" && object._parent) {
      // Preserve Group's bitmap/autowidth fallback and other native overrides.
      value += object._parent[axis === "w" ? "getwidth" : "getheight"]();
    }
    const suffix = axis === "w" ? "Width" : "Height";
    value = Math.max(0, Number(object[`_minimum${suffix}`]) || 0, value);
    const maximum = Number(object[`_maximum${suffix}`]) || 0;
    return Math.trunc(maximum > 0 ? Math.min(value, maximum) : value);
  } finally {
    seen.delete(object);
  }
}

export function installMakiGeometry() {
  // Native Layout::move/resize keeps the GUI coordinates in screen space.
  // The browser layout itself is at (0,0) inside its positioned container.
  Layout.prototype.getguix = function () { return this.getleft(); };
  Layout.prototype.getguiy = function () { return this.gettop(); };
  const setAttribute = GuiObj.prototype.setXmlAttr;
  GuiObj.prototype.setXmlAttr = function (key: string, value: string) {
    if (key.toLowerCase() !== "fitparent") return setAttribute.call(this, key, value);
    const fit = Number.parseInt(value, 10) || 0;
    if (fit) {
      const inset = Math.max(0, -fit);
      this._x = inset; this._y = inset;
      this._w = inset ? -2 * inset : 0; this._h = inset ? -2 * inset : 0;
      this._relatw = "1"; this._relath = "1";
      this._renderDimensions();
    }
    return true;
  };
  GuiObj.prototype.getwidth = function () { return dimension(this, "w"); };
  GuiObj.prototype.getheight = function () { return dimension(this, "h"); };
  for (const [method, axis] of [["getleft", "x"], ["gettop", "y"]] as const) {
    GuiObj.prototype[method] = function () {
      const object = this as any;
      const offset = Number(object[`_${axis}`]) || 0;
      return offset + (String(object[`_relat${axis}`]) === "1" && object._parent
        ? object._parent[axis === "x" ? "getwidth" : "getheight"]() : 0);
    };
  }
  // Kog hosts every skin window inside one browser viewport. Its origin is
  // therefore the application origin in the skin's coordinate system.
  SystemObject.prototype.getcurappwidth = function () { return document.documentElement.clientWidth; };
  SystemObject.prototype.getcurappheight = function () { return document.documentElement.clientHeight; };
  SystemObject.prototype.getcurappleft = function () { return 0; };
  SystemObject.prototype.getcurapptop = function () { return 0; };
  const system = SystemObject.prototype as any;
  for (const suffix of ["", "fromguiobject", "frompoint"]) {
    const arity = suffix === "fromguiobject" ? 1 : suffix === "frompoint" ? 2 : 0;
    for (const [axis, read] of Object.entries({
      width: () => document.documentElement.clientWidth,
      height: () => document.documentElement.clientHeight,
      left: () => 0, top: () => 0,
    })) {
      // The interpreter validates the callable's declared MAKI argument count.
      const method = arity === 1 ? function (_object: unknown) { return read(); }
        : arity === 2 ? function (_x: number, _y: number) { return read(); } : read;
      system[`getviewport${axis}${suffix}`] = method;
    }
  }
}
