import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import path from "node:path";
import esbuild from "esbuild";

const directory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

async function loadFixture() {
  const result = await esbuild.build({
    bundle: true,
    format: "esm",
    platform: "node",
    write: false,
    stdin: {
      loader: "ts",
      resolveDir: directory,
      sourcefile: "classicpro-graphics-fixture.ts",
      contents: `
        import { installClassicProGraphics } from "./src/classicpro-graphics.ts";
        import Layer from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Layer.ts";
        import MakiMap from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/MakiMap.ts";
        export { installClassicProGraphics, Layer, MakiMap };
      `,
    },
  });
  const source = Buffer.from(result.outputFiles[0].contents).toString("base64");
  return import(`data:text/javascript;base64,${source}`);
}

function imageData(width, height, pixels = []) {
  const data = new Uint8ClampedArray(width * height * 4);
  pixels.forEach((pixel, index) => data.set(pixel, index * 4));
  return { width, height, data };
}

function canvasFor(source, dataUrl = "data:image/png;base64,mask") {
  let written = null;
  return {
    width: source.width,
    height: source.height,
    get written() { return written; },
    getContext() {
      return {
        createImageData: (width, height) => imageData(width, height),
        drawImage() {},
        getImageData(x, y, width, height) {
          if (x === 0 && y === 0 && width === source.width && height === source.height) return source;
          const offset = (x + y * source.width) * 4;
          return imageData(1, 1, [source.data.slice(offset, offset + 4)]);
        },
        putImageData(value) { written = value; },
      };
    },
    toDataURL: () => dataUrl,
  };
}

function bitmap(canvas, { loaded = true, width = canvas.width, height = canvas.height } = {}) {
  return {
    getCanvas: () => canvas,
    getHeight: () => height,
    getImg: () => loaded ? {} : null,
    getWidth: () => width,
    loaded: () => loaded,
  };
}

function rootWith(bitmaps, aliases = {}) {
  return {
    getAlias: id => aliases[id],
    getBitmaps: () => bitmaps,
  };
}

function fakeStyle() {
  const values = new Map();
  return {
    values,
    clipPath: "",
    setProperty(name, value) { values.set(name, value); },
  };
}

test("ClassicPro Map exposes safe dimensions and Winamp BGRA/premultiplied values", async () => {
  const { installClassicProGraphics, MakiMap } = await loadFixture();
  installClassicProGraphics();

  const source = imageData(2, 1, [
    [20, 80, 200, 255],
    [255, 64, 32, 128],
  ]);
  const mapBitmap = bitmap(canvasFor(source));
  const map = new MakiMap(rootWith({ colors: mapBitmap }, { alias: "colors" }));

  map.loadmap("ALIAS");
  assert.equal(map.getwidth(), 2);
  assert.equal(map.getheight(), 1);
  assert.equal(map.getvalue(0, 0), 200);
  assert.deepEqual([0, 1, 2, 3].map(channel => map.getargbvalue(0, 0, channel)), [200, 80, 20, 255]);
  // Winamp premultiplies with >> 8, then getARGBValue corrects with ceil().
  assert.deepEqual([0, 1, 2, 3].map(channel => map.getargbvalue(1, 0, channel)), [32, 64, 254, 128]);
  assert.equal(map.getvalue(1, 0), 127);
  assert.equal(map.inregion(1, 0), true);
  assert.equal(map.getargbvalue(8, 0, 2), 0);

  map.loadmap("missing");
  assert.equal(map.getwidth(), 0);
  assert.equal(map.getheight(), 0);
  assert.equal(map.getvalue(0, 0), 0);
  assert.equal(map.inregion(0, 0), false);
});

test("ClassicPro Map awaits images supplied only by the prepared resource extractor", async () => {
  const { installClassicProGraphics, MakiMap } = await loadFixture();
  installClassicProGraphics();

  const requested = [];
  const installedBytes = new Uint8Array(await readFile(
    path.join(directory, "vendor/classicpro/engine/image/installed.png"),
  ));
  const versionBytes = new Uint8Array(await readFile(
    path.join(directory, "vendor/classicpro/engine/image/version.png"),
  ));
  const root = rootWith({});
  root._fileExtractor = {
    resource(resourcePath) {
      requested.push(resourcePath);
      if (resourcePath.includes("installed.png")) return installedBytes;
      if (resourcePath.includes("version.png")) return versionBytes;
      return null;
    },
  };

  const decodedPixels = imageData(1, 1, [
    [9, 8, 7, 255],
  ]);
  const decodedCanvas = canvasFor(decodedPixels);
  const priorCreateImageBitmap = globalThis.createImageBitmap;
  const priorDocument = globalThis.document;
  const priorFetch = globalThis.fetch;
  const priorHtmlCanvas = globalThis.HTMLCanvasElement;
  let fetchCalled = false;
  const decodedDimensions = [];
  globalThis.HTMLCanvasElement = class FakeHtmlCanvasElement {};
  globalThis.document = { createElement: () => decodedCanvas };
  globalThis.fetch = () => { fetchCalled = true; throw new Error("unexpected fetch"); };
  globalThis.createImageBitmap = async blob => {
    const encoded = new Uint8Array(await blob.arrayBuffer());
    assert.deepEqual([...encoded.slice(0, 8)], [137, 80, 78, 71, 13, 10, 26, 10]);
    const view = new DataView(encoded.buffer, encoded.byteOffset, encoded.byteLength);
    const dimensions = { width: view.getUint32(16), height: view.getUint32(20) };
    decodedDimensions.push(dimensions);
    return dimensions;
  };

  try {
    const map = new MakiMap(root);
    const imagePath = "@COLORTHEMESPATH@\\..\\..\\Plugins\\classicPro\\engine\\image\\installed.png";
    const pending = map.loadmap(imagePath);
    assert.equal(map.getwidth(), 0, "the map remains invalid until decoding finishes");
    await pending;
    assert.equal(map.getwidth(), 1);
    assert.equal(map.getheight(), 1);
    assert.equal(map.getargbvalue(0, 0, 0), 7);

    const versionPath = "@COLORTHEMESPATH@\\..\\..\\Plugins\\classicPro\\engine\\image\\version.png";
    await map.loadmap(versionPath);
    assert.equal(map.getwidth(), 2);
    assert.equal(map.getheight(), 3);

    await map.loadmap("https://example.invalid/image.png");
    assert.equal(map.getwidth(), 0);
    assert.equal(fetchCalled, false);
    assert.deepEqual(decodedDimensions, [{ width: 1, height: 1 }, { width: 2, height: 3 }]);
    assert.deepEqual(requested, [imagePath, versionPath, "https://example.invalid/image.png"]);
  } finally {
    globalThis.createImageBitmap = priorCreateImageBitmap;
    globalThis.document = priorDocument;
    globalThis.fetch = priorFetch;
    globalThis.HTMLCanvasElement = priorHtmlCanvas;
  }
});

test("ClassicPro Layer reports failed images and applies an exact threshold mask", async () => {
  const { installClassicProGraphics, Layer, MakiMap } = await loadFixture();
  installClassicProGraphics();

  const source = imageData(4, 1, [
    [255, 255, 255, 255],
    [254, 255, 255, 255],
    [0, 0, 0, 255],
    [255, 255, 255, 0],
  ]);
  const sourceCanvas = canvasFor(source);
  const valid = bitmap(sourceCanvas);
  const failed = bitmap(sourceCanvas, { loaded: false });
  const root = rootWith({ valid, failed });

  const validLayer = Object.create(Layer.prototype);
  validLayer._uiRoot = root;
  validLayer._image = "valid";
  validLayer._div = { style: fakeStyle() };
  assert.equal(validLayer.isinvalid(), false);

  const failedLayer = Object.create(Layer.prototype);
  failedLayer._uiRoot = root;
  failedLayer._image = "failed";
  assert.equal(failedLayer.isinvalid(), true);
  failedLayer._image = "absent";
  assert.equal(failedLayer.isinvalid(), true);

  const maskCanvas = canvasFor(imageData(0, 0), "data:image/png;base64,exact-mask");
  const priorDocument = globalThis.document;
  globalThis.document = { createElement(name) { assert.equal(name, "canvas"); return maskCanvas; } };
  try {
    const map = new MakiMap(root);
    map.loadmap("valid");
    validLayer.setregionfrommap(map, 255, false);
  } finally {
    globalThis.document = priorDocument;
  }

  assert.deepEqual([...maskCanvas.written.data], [
    255, 255, 255, 255,
    0, 0, 0, 0,
    0, 0, 0, 0,
    0, 0, 0, 0,
  ]);
  assert.equal(validLayer._div.style.values.get("mask-image"), 'url("data:image/png;base64,exact-mask")');
  assert.equal(validLayer._div.style.values.get("mask-repeat"), "no-repeat");
  assert.equal(validLayer._div.style.clipPath, 'path("M0 0H1V1H0Z")');
});

test("ClassicPro reversed regions select dark opaque pixels", async () => {
  const { installClassicProGraphics, Layer, MakiMap } = await loadFixture();
  installClassicProGraphics();
  const source = imageData(3, 1, [
    [20, 20, 20, 255],
    [21, 20, 20, 255],
    [0, 0, 0, 0],
  ]);
  const root = rootWith({ map: bitmap(canvasFor(source)) });
  const map = new MakiMap(root);
  map.loadmap("map");
  const layer = Object.create(Layer.prototype);
  layer._div = { style: fakeStyle() };

  const maskCanvas = canvasFor(imageData(0, 0));
  const priorDocument = globalThis.document;
  globalThis.document = { createElement: () => maskCanvas };
  try {
    layer.setregionfrommap(map, 20, true);
  } finally {
    globalThis.document = priorDocument;
  }
  assert.deepEqual([...maskCanvas.written.data], [
    255, 255, 255, 255,
    0, 0, 0, 0,
    0, 0, 0, 0,
  ]);
});
