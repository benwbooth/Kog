import test from "node:test";
import assert from "node:assert/strict";
import { isLibraryComponent, intersectRect, libraryViewport, publishLibraryViewport } from "../src/host-library.js";

test("library slots match the media library GUID, not arbitrary components", () => {
  assert.equal(isLibraryComponent("guid:{6B0EDF80-C9A5-11D3-9F26-00C04F39FFC6}"), true);
  assert.equal(isLibraryComponent("guid:ml"), true);
  assert.equal(isLibraryComponent("guid:pl"), false);
  assert.equal(isLibraryComponent(undefined), false);
});

test("native library rectangles intersect viewport and clipping parents", () => {
  assert.deepEqual(intersectRect({ x: -10, y: 5, width: 40, height: 60 },
    { x: 0, y: 0, width: 100, height: 50 }), { x: 0, y: 5, width: 30, height: 45 });
  assert.equal(intersectRect({ x: 200, y: 5, width: 40, height: 60 },
    { x: 0, y: 0, width: 100, height: 50 }), null);
});

test("library geometry refresh recovers a dropped host update and stops on page exit", () => {
  const document = { querySelector: () => null, querySelectorAll: () => [] };
  let tick, pagehide, cleared;
  const window = {
    innerWidth: 100, innerHeight: 100,
    setInterval(callback, delay) { tick = callback; assert.equal(delay, 100); return 123; },
    clearInterval(id) { cleared = id; },
    addEventListener(event, callback) { assert.equal(event, "pagehide"); pagehide = callback; },
  };
  const calls = [];
  publishLibraryViewport((...args) => calls.push(args), document, window);
  assert.deepEqual(calls, [["libraryViewport", null]]);
  for (let i = 0; i < 9; ++i) tick();
  assert.equal(calls.length, 1);
  tick();
  assert.deepEqual(calls, [["libraryViewport", null], ["libraryViewport", null]]);
  pagehide();
  assert.equal(cleared, 123);
});

test("library viewport hides inactive slots and popup menus, clips and rounds inward", () => {
  const style = { display: "block", visibility: "visible", opacity: "1", overflowX: "visible", overflowY: "visible" };
  const parent = { style: { ...style, overflowX: "hidden" }, parentElement: null,
    getBoundingClientRect: () => ({ x: 0, y: 0, width: 80, height: 100 }) };
  const element = { style: { ...style }, parentElement: parent, contains: () => false,
    getBoundingClientRect: () => ({ x: 10.1, y: 10.1, width: 100, height: 50.6 }) };
  const popup = { ...element, style: { ...style, display: "none" } };
  const document = { querySelectorAll: selector => selector === ".popup-menu-container" ? [popup] : [element],
    defaultView: { getComputedStyle: element => element.style }, elementFromPoint: () => element };
  const viewport = { x: 0, y: 0, width: 100, height: 100 };
  assert.deepEqual(libraryViewport(document, viewport), { x: 11, y: 11, width: 69, height: 49 });
  parent.style.display = "none";
  assert.equal(libraryViewport(document, viewport), null);
  parent.style.display = "block";
  popup.style.display = "block";
  assert.equal(libraryViewport(document, viewport), null);
});
