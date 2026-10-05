import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { bindPlaylistReorder, playlistDropTarget } from "../src/playlist-reorder.js";

const webTarget = new Function("document", "x", "y", "queueLength",
  await readFile(new URL("../../../crates/kog-web/src/track-drop.js", import.meta.url), "utf8"));

function fixture(indices = [0, 1, 2]) {
  const rows = indices.map((index, i) => {
    const classes = new Set();
    return {
      dataset: { index: String(index) }, classes,
      classList: { add: value => classes.add(value), remove: (...values) => values.forEach(value => classes.delete(value)) },
      getBoundingClientRect: () => ({ top: 60 + i * 24, height: 24 }),
    };
  });
  const listeners = new Map();
  const pane = {
    clientWidth: 300, clientHeight: 200,
    getBoundingClientRect: () => ({ left: 10, top: 40 }),
    querySelectorAll: () => rows,
    querySelector: () => ({ getBoundingClientRect: () => ({ bottom: 60 }) }),
    contains: target => target === pane || rows.includes(target),
    addEventListener: (type, listener) => listeners.set(type, listener),
  };
  const document = { getElementById: () => pane };
  const dispatch = (type, y, extra = {}) => {
    const event = { clientX: 100, clientY: y, dataTransfer: { types: ["text/x-kog-playlist"] },
      preventDefault() { this.prevented = true; }, stopPropagation() {}, ...extra };
    listeners.get(type)(event);
    return event;
  };
  return { pane, rows, document, dispatch };
}

for (const frontend of ["web", "modern skin"]) {
  test(`${frontend}: insertion marker covers first, middle and final gaps without stale lines`, () => {
    const { pane, rows, document } = fixture();
    const target = (x, y) => frontend === "web" ? webTarget(document, x, y, 3) : playlistDropTarget(pane, x, y);
    const before = frontend === "web" ? "reorder-above" : "kog-reorder-before";
    const after = frontend === "web" ? "reorder-below" : "kog-reorder-after";
    assert.equal(target(100, 65), 0);
    assert.ok(rows[0].classes.has(before));
    assert.equal(target(100, 90), 1);
    assert.equal(rows[0].classes.size, 0);
    assert.ok(rows[1].classes.has(before));
    assert.equal(target(100, 180), 3);
    assert.equal(rows[1].classes.size, 0);
    assert.ok(rows[2].classes.has(after));
    assert.equal(target(5, 180), null);
    assert.ok(rows.every(row => row.classes.size === 0));
    assert.equal(target(100, 250), null);
  });
}

test("web: filtered rows return the source index at the marked gap", () => {
  const { rows, document } = fixture([2, 5, 9]);
  assert.equal(webTarget(document, 100, 90, 12), 5);
  assert.ok(rows[1].classes.has("reorder-above"));
  assert.equal(webTarget(document, 100, 180, 12), 12);
  assert.ok(rows[2].classes.has("reorder-below"));
  assert.equal(webTarget(document, 100, 45, 12), null, "Header is not a drop target");
  assert.ok(rows.every(row => row.classes.size === 0));
});

test("modern skin: drop uses the displayed gap and cancel/leave removes the indicator", () => {
  const { pane, rows, dispatch } = fixture();
  const moves = [];
  bindPlaylistReorder(pane, index => moves.push(index));
  assert.ok(dispatch("dragover", 180).prevented);
  assert.ok(rows[2].classes.has("kog-reorder-after"));
  dispatch("drop", 180);
  assert.deepEqual(moves, [3]);
  assert.ok(rows.every(row => row.classes.size === 0));
  dispatch("dragover", 90);
  dispatch("dragleave", 90, { relatedTarget: rows[2] });
  assert.ok(rows[1].classes.has("kog-reorder-before"), "Moving over another row keeps the marker");
  dispatch("dragleave", 90, { relatedTarget: null });
  assert.ok(rows.every(row => row.classes.size === 0));
  dispatch("dragover", 90);
  dispatch("dragend", 90);
  assert.ok(rows.every(row => row.classes.size === 0));
  dispatch("drop", 90, { dataTransfer: { types: ["text/plain"] } });
  assert.deepEqual(moves, [3], "An unrelated drag must not reorder tracks");
});
