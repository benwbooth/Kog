import Bitmap from "../../../native/webamp/packages/webamp-modern/src/skin/Bitmap";
import Layer from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Layer";
import MakiMap from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/MakiMap";

type CompatibleMap = InstanceType<typeof MakiMap> & {
  _bitmap?: Bitmap | null;
};

type CompatibleLayer = InstanceType<typeof Layer> & {
  _backgroundBitmap?: Bitmap | null;
  _image?: string;
};

type Pixel = readonly [red: number, green: number, blue: number, alpha: number];

let installed = false;
let nextMapLoad = 0;
const mapLoads = new WeakMap<object, number>();
const MAX_HIT_TEST_RUNS = 4096;

function findBitmap(root: any, id: unknown): Bitmap | null {
  const key = String(id ?? "").toLowerCase();
  if (!key || !root || typeof root.getBitmaps !== "function") return null;

  const bitmaps = root.getBitmaps() as Record<string, Bitmap> | null;
  if (!bitmaps) return null;
  if (Object.prototype.hasOwnProperty.call(bitmaps, key)) return bitmaps[key] ?? null;

  // UIRoot.getBitmap supports one level of element aliasing, but warns and
  // returns undefined for a missing bitmap. Resolve that same alias without
  // making an optional ClassicPro image a renderer diagnostic.
  const alias = typeof root.getAlias === "function" ? root.getAlias(key) : null;
  const aliasKey = alias ? String(alias).toLowerCase() : "";
  return aliasKey && Object.prototype.hasOwnProperty.call(bitmaps, aliasKey)
    ? bitmaps[aliasKey] ?? null
    : null;
}

function currentBitmap(map: CompatibleMap): Bitmap | null {
  return map._bitmap ?? null;
}

function loadedCanvas(bitmap: Bitmap | null): HTMLCanvasElement | null {
  if (!bitmap || typeof bitmap.loaded !== "function" || !bitmap.loaded()) return null;
  try {
    return bitmap.getCanvas(true);
  } catch {
    return null;
  }
}

function bitmapDimension(bitmap: Bitmap | null, dimension: "width" | "height"): number {
  if (!bitmap) return 0;
  const declared = dimension === "width" ? bitmap.getWidth() : bitmap.getHeight();
  if (Number.isFinite(declared) && declared >= 0) return Math.trunc(declared);
  const canvas = loadedCanvas(bitmap);
  return canvas ? canvas[dimension] : 0;
}

function pixelAt(map: CompatibleMap, xValue: unknown, yValue: unknown): Pixel | null {
  const canvas = loadedCanvas(currentBitmap(map));
  const x = Math.trunc(Number(xValue));
  const y = Math.trunc(Number(yValue));
  if (!canvas || !Number.isFinite(x) || !Number.isFinite(y) ||
      x < 0 || y < 0 || x >= canvas.width || y >= canvas.height) {
    return null;
  }

  try {
    const data = canvas.getContext("2d")?.getImageData(x, y, 1, 1).data;
    return data ? [data[0], data[1], data[2], data[3]] : null;
  } catch {
    return null;
  }
}

// Winamp's image loader stores premultiplied channels using (channel * alpha)
// >> 8. Canvas getImageData() returns unpremultiplied RGBA, so reproduce that
// storage representation before applying the original Map algorithms.
function premultiplied(channel: number, alpha: number): number {
  return alpha === 255 ? channel : (channel * alpha) >> 8;
}

async function decodeResourceImage(bytes: Uint8Array): Promise<CanvasImageSource | null> {
  const blob = new Blob([bytes]);
  if (typeof createImageBitmap === "function") {
    try {
      return await createImageBitmap(blob);
    } catch {
      // Older QtWebEngine builds can expose createImageBitmap but reject a
      // format which HTMLImageElement can still decode.
    }
  }

  if (typeof Image !== "function" || typeof URL?.createObjectURL !== "function") return null;
  const url = URL.createObjectURL(blob);
  try {
    return await new Promise<HTMLImageElement | null>((resolve) => {
      const image = new Image();
      image.addEventListener("load", () => resolve(image), { once: true });
      image.addEventListener("error", () => resolve(null), { once: true });
      image.src = url;
    });
  } finally {
    URL.revokeObjectURL(url);
  }
}

function closeDecodedImage(image: CanvasImageSource | null): void {
  if (typeof ImageBitmap !== "undefined" && image instanceof ImageBitmap) image.close();
}

async function loadmap(this: CompatibleMap, bitmapId: string): Promise<void> {
  const root = (this as any)._uiRoot;
  const existing = findBitmap(root, bitmapId);
  const load = ++nextMapLoad;
  mapLoads.set(this, load);
  this._bitmap = existing;
  if (existing) return;

  // Runtime extractors are the sole resource authority. In particular, do
  // not call UIRoot.getFileAsBlob(): generic Webamp extractors may implement
  // it with fetch(), while Kog's resource() exposes only prepared archive and
  // bundled ClassicPro bytes.
  const extractor = root?._fileExtractor;
  let bytes: unknown = null;
  try {
    bytes = typeof extractor?.resource === "function"
      ? extractor.resource(String(bitmapId ?? ""))
      : null;
  } catch {
    bytes = null;
  }
  if (!(bytes instanceof Uint8Array) || bytes.byteLength === 0) return;

  const image = await decodeResourceImage(bytes);
  if (mapLoads.get(this) !== load) {
    closeDecodedImage(image);
    return;
  }
  const width = Number((image as { width?: unknown } | null)?.width);
  const height = Number((image as { height?: unknown } | null)?.height);
  if (!image || !Number.isSafeInteger(width) || !Number.isSafeInteger(height) ||
      width <= 0 || height <= 0) {
    closeDecodedImage(image);
    return;
  }

  const bitmap = new Bitmap(root);
  bitmap.setXmlAttributes({
    id: `__kog_map__${String(bitmapId ?? "")}`,
    file: String(bitmapId ?? ""),
    w: String(width),
    h: String(height),
  });
  bitmap.setImage(image);
  this._bitmap = bitmap;
}

function getwidth(this: CompatibleMap): number {
  return bitmapDimension(currentBitmap(this), "width");
}

function getheight(this: CompatibleMap): number {
  return bitmapDimension(currentBitmap(this), "height");
}

function getvalue(this: CompatibleMap, x: number, y: number): number {
  const pixel = pixelAt(this, x, y);
  if (!pixel) return 0;
  const [red, green, blue, alpha] = pixel;
  return Math.max(
    premultiplied(red, alpha),
    premultiplied(green, alpha),
    premultiplied(blue, alpha),
  );
}

function getargbvalue(
  this: CompatibleMap,
  x: number,
  y: number,
  channelValue: number,
): number {
  const pixel = pixelAt(this, x, y);
  if (!pixel) return 0;

  const channel = ((Math.trunc(Number(channelValue)) % 4) + 4) % 4;
  const alpha = pixel[3];
  if (channel === 3) return alpha;

  // MAKI channel order is BGRA, whereas ImageData is RGBA.
  const source = pixel[channel === 0 ? 2 : channel === 1 ? 1 : 0];
  const stored = premultiplied(source, alpha);
  if (alpha === 0 || alpha === 255) return stored;
  return Math.ceil((stored * 255) / alpha);
}

function inregion(this: CompatibleMap, x: number, y: number): boolean {
  return (pixelAt(this, x, y)?.[3] ?? 0) > 0;
}

function isinvalid(this: CompatibleLayer): boolean {
  const image = this._image;
  const bitmap = image
    ? findBitmap((this as any)._uiRoot, image)
    : this._backgroundBitmap ?? null;
  return !bitmap || typeof bitmap.loaded !== "function" || !bitmap.loaded() || !bitmap.getImg();
}

function setStyleMask(layer: CompatibleLayer, url: string, clipPath: string): void {
  const style = (layer as any)._div?.style as CSSStyleDeclaration | undefined;
  if (!style) return;
  const image = `url(${JSON.stringify(url)})`;
  style.setProperty("mask-image", image);
  style.setProperty("-webkit-mask-image", image);
  style.setProperty("mask-repeat", "no-repeat");
  style.setProperty("-webkit-mask-repeat", "no-repeat");
  style.setProperty("mask-position", "0 0");
  style.setProperty("-webkit-mask-position", "0 0");
  style.setProperty("mask-mode", "alpha");
  style.clipPath = clipPath;
}

function emptyRegion(layer: CompatibleLayer): void {
  // An invalid native bitmap produces an empty RegionI, rather than leaving
  // the layer un-clipped. This data URL is intentionally synchronous: MAKI
  // expects setRegionFromMap() to have taken effect when the call returns.
  setStyleMask(
    layer,
    "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='1' height='1'/%3E",
    "polygon(0 0, 0 0, 0 0)",
  );
}

function setregionfrommap(
  this: CompatibleLayer,
  map: CompatibleMap,
  thresholdValue: number,
  reversedValue: boolean,
): void {
  const canvas = map ? loadedCanvas(currentBitmap(map)) : null;
  if (!canvas || canvas.width <= 0 || canvas.height <= 0) {
    emptyRegion(this);
    return;
  }

  const sourceContext = canvas.getContext("2d");
  let source: ImageData;
  try {
    if (!sourceContext) throw new Error("Map canvas has no 2D context");
    source = sourceContext.getImageData(0, 0, canvas.width, canvas.height);
  } catch {
    emptyRegion(this);
    return;
  }

  const maskCanvas = document.createElement("canvas");
  maskCanvas.width = canvas.width;
  maskCanvas.height = canvas.height;
  const maskContext = maskCanvas.getContext("2d");
  if (!maskContext) {
    emptyRegion(this);
    return;
  }

  const threshold = Math.trunc(Number(thresholdValue)) & 0xff;
  const reversed = Boolean(reversedValue);
  const mask = maskContext.createImageData(canvas.width, canvas.height);
  const path: string[] = [];
  let pathOverflow = false;

  for (let y = 0; y < canvas.height; y += 1) {
    let runStart = -1;
    for (let x = 0; x <= canvas.width; x += 1) {
      let included = false;
      if (x < canvas.width) {
        const offset = (x + y * canvas.width) * 4;
        const alpha = source.data[offset + 3];
        if (alpha > 0) {
          const red = premultiplied(source.data[offset], alpha);
          const green = premultiplied(source.data[offset + 1], alpha);
          const blue = premultiplied(source.data[offset + 2], alpha);
          included = reversed
            ? red <= threshold && green <= threshold && blue <= threshold
            : red >= threshold && green >= threshold && blue >= threshold;
        }
        if (included) {
          mask.data[offset] = 255;
          mask.data[offset + 1] = 255;
          mask.data[offset + 2] = 255;
          mask.data[offset + 3] = 255;
        }
      }

      if (included && runStart < 0) runStart = x;
      if (!included && runStart >= 0) {
        // A subpath per horizontal run is an exact hit-test clip in browsers
        // that support CSS path(); mask-image remains the exact visual clip.
        if (path.length < MAX_HIT_TEST_RUNS) {
          path.push(`M${runStart} ${y}H${x}V${y + 1}H${runStart}Z`);
        } else {
          // Do not let a deliberately noisy skin bitmap create an unbounded
          // CSS declaration. The canvas mask remains pixel-exact visually.
          pathOverflow = true;
        }
        runStart = -1;
      }
    }
  }

  maskContext.putImageData(mask, 0, 0);
  const clipPath = pathOverflow
    ? "none"
    : path.length
      ? `path(${JSON.stringify(path.join(""))})`
      : "polygon(0 0, 0 0, 0 0)";
  setStyleMask(this, maskCanvas.toDataURL("image/png"), clipPath);
}

/** Install the ClassicPro Map and Layer subset on Webamp's MAKI classes. */
export function installClassicProGraphics(): void {
  if (installed) return;
  installed = true;

  MakiMap.prototype.loadmap = loadmap;
  MakiMap.prototype.getwidth = getwidth;
  MakiMap.prototype.getheight = getheight;
  MakiMap.prototype.getvalue = getvalue;
  MakiMap.prototype.getUnsafeValue = function (x: number, y: number): number | null {
    const pixel = pixelAt(this as CompatibleMap, x, y);
    if (!pixel || pixel[0] !== pixel[1] || pixel[0] !== pixel[2] || pixel[3] !== 255) {
      return null;
    }
    return pixel[0];
  };
  MakiMap.prototype.inregion = inregion;
  (MakiMap.prototype as any).getargbvalue = getargbvalue;
  (Layer.prototype as any).isinvalid = isinvalid;
  (Layer.prototype as any).setregionfrommap = setregionfrommap;
}
