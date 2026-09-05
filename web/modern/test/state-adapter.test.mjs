import assert from "node:assert/strict";
import test from "node:test";

import {
  CommandGateway,
  findSkinPrefix,
  moveTargetAfterRemoval,
  normalizeArchivePath,
  sanitizeState,
  StateStore,
} from "../src/state-adapter.js";

test("state updates retain playlist rows when tracks are omitted", () => {
  const store = new StateStore();
  const first = store.applyJson(
    JSON.stringify({
      playback: "playing",
      position: 4,
      duration: 40,
      volume: 0.5,
      currentIndex: 0,
      revision: 9,
      tracks: [
        {
          id: "track-1",
          title: "A <b>literal</b> title",
          artist: "Artist",
          album: "Album",
          duration: 40,
        },
      ],
      shuffle: "all",
      repeat: "playlist",
      songInfo: "FLAC • 96 kHz • stereo • 2841 kbps",
    }),
  );
  const second = store.applyJson(
    JSON.stringify({
      playback: "paused",
      position: 7,
      duration: 40,
      volume: 0.5,
      currentIndex: 0,
      revision: 9,
      shuffle: "all",
      repeat: "playlist",
    }),
  );

  assert.equal(second.tracks, first.tracks);
  assert.equal(second.tracks[0].title, "A <b>literal</b> title");
  assert.equal(second.playback, "paused");
  assert.equal(second.position, 7);
  assert.equal(second.songInfo, "FLAC • 96 kHz • stereo • 2841 kbps");
});

test("state normalizer clamps host-controlled values and bounds PCM", () => {
  const state = sanitizeState({
    volume: 20,
    position: -10,
    duration: Number.NaN,
    eq: [-99, 99],
    eqPreamp: 90,
    visualization: {
      wave: new Array(5_000).fill(0.25),
      spectrum: [0, Number.NaN, 2],
    },
  });

  assert.equal(state.volume, 1);
  assert.equal(state.position, 0);
  assert.equal(state.duration, 0);
  assert.deepEqual(state.eq.slice(0, 3), [-12, 12, 0]);
  assert.equal(state.eqPreamp, 12);
  assert.equal(state.visualization.wave.length, 4_096);
  assert.deepEqual(state.visualization.spectrum, [0, 0, 2]);
});

test("destructive bridge commands require current user activation", () => {
  const calls = [];
  let active = false;
  const commands = new CommandGateway(
    (command, payload) => calls.push([command, payload]),
    () => active,
  );

  assert.equal(commands.send("clear"), false);
  assert.equal(commands.send("swap", { first: 0, second: 1 }), false);
  assert.equal(commands.send("play"), true);
  active = true;
  assert.equal(commands.send("remove", [1, 2]), true);
  assert.equal(commands.send("swap", { first: 0, second: 1 }), true);

  assert.deepEqual(calls, [
    ["play", "null"],
    ["remove", "[1,2]"],
    ["swap", '{"first":0,"second":1}'],
  ]);
});

test("archive prefix accepts one wrapper directory and rejects ambiguity", () => {
  assert.equal(findSkinPrefix(["skin.xml", "player.png"]), "");
  assert.equal(
    findSkinPrefix(["Wrapper/SKIN.XML", "Wrapper/player.png"]),
    "Wrapper/",
  );
  assert.throws(
    () => findSkinPrefix(["one/skin.xml", "two/skin.xml"]),
    /exactly one/,
  );
  assert.throws(() => normalizeArchivePath("../skin.xml"), /Unsafe/);
  assert.throws(() => normalizeArchivePath("/skin.xml"), /Unsafe/);
});

test("playlist drop target uses the after-removal coordinate space", () => {
  assert.equal(moveTargetAfterRemoval([1], 4), 3);
  assert.equal(moveTargetAfterRemoval([1, 2], 5), 3);
  assert.equal(moveTargetAfterRemoval([4], 1), 1);
  assert.equal(moveTargetAfterRemoval([2, 2], 5), 4);
});
