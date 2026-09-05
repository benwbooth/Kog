const PLAYBACK = new Set(["playing", "paused", "stopped"]);
const SHUFFLE = new Set(["off", "all", "albums"]);
const REPEAT = new Set(["off", "playlist", "track"]);

const MAX_TRACKS = 100_000;
const MAX_VIS_SAMPLES = 4_096;
const MAX_STATE_JSON_BYTES = 16 * 1024 * 1024;

function finite(value, fallback = 0) {
  const number = Number(value);
  return Number.isFinite(number) ? number : fallback;
}

function clamp(value, min, max) {
  return Math.min(max, Math.max(min, value));
}

function stringValue(value, fallback = "") {
  return typeof value === "string" ? value : fallback;
}

function sanitizeTrack(track, index) {
  const value = track && typeof track === "object" ? track : {};
  return Object.freeze({
    id: stringValue(value.id, String(index)),
    title: stringValue(value.title),
    artist: stringValue(value.artist),
    album: stringValue(value.album),
    duration: Math.max(0, finite(value.duration)),
  });
}

function sanitizeSamples(samples) {
  if (!Array.isArray(samples)) return [];
  return samples
    .slice(0, MAX_VIS_SAMPLES)
    .map((sample) => finite(sample));
}

function sanitizeEq(eq, previous) {
  if (!Array.isArray(eq)) return previous;
  const values = new Array(10).fill(0);
  for (let index = 0; index < values.length; index += 1) {
    values[index] = clamp(finite(eq[index]), -12, 12);
  }
  return Object.freeze(values);
}

export function initialState() {
  return Object.freeze({
    playback: "stopped",
    position: 0,
    duration: 0,
    volume: 1,
    currentIndex: -1,
    revision: -1,
    tracks: Object.freeze([]),
    shuffle: "off",
    repeat: "off",
    eq: Object.freeze(new Array(10).fill(0)),
    eqEnabled: false,
    eqPreamp: 0,
    songInfo: "",
    visualization: Object.freeze({
      wave: Object.freeze([]),
      spectrum: Object.freeze([]),
    }),
  });
}

export function sanitizeState(value, previous = initialState()) {
  const input = value && typeof value === "object" ? value : {};
  const hasTracks = Array.isArray(input.tracks);
  const tracks = hasTracks
    ? Object.freeze(input.tracks.slice(0, MAX_TRACKS).map(sanitizeTrack))
    : previous.tracks;
  const visualization =
    input.visualization && typeof input.visualization === "object"
      ? Object.freeze({
          wave: Object.freeze(sanitizeSamples(input.visualization.wave)),
          spectrum: Object.freeze(
            sanitizeSamples(input.visualization.spectrum),
          ),
        })
      : previous.visualization;

  return Object.freeze({
    playback: PLAYBACK.has(input.playback)
      ? input.playback
      : previous.playback,
    position: Math.max(0, finite(input.position, previous.position)),
    duration: Math.max(0, finite(input.duration, previous.duration)),
    volume: clamp(finite(input.volume, previous.volume), 0, 1),
    currentIndex: Math.trunc(finite(input.currentIndex, previous.currentIndex)),
    revision: Math.trunc(finite(input.revision, previous.revision)),
    tracks,
    shuffle: SHUFFLE.has(input.shuffle) ? input.shuffle : previous.shuffle,
    repeat: REPEAT.has(input.repeat) ? input.repeat : previous.repeat,
    eq: sanitizeEq(input.eq, previous.eq),
    eqEnabled:
      typeof input.eqEnabled === "boolean"
        ? input.eqEnabled
        : previous.eqEnabled,
    eqPreamp: clamp(finite(input.eqPreamp, previous.eqPreamp), -12, 12),
    songInfo:
      typeof input.songInfo === "string"
        ? input.songInfo.slice(0, 1_024)
        : previous.songInfo,
    visualization,
  });
}

export class StateStore {
  #state = initialState();
  #listeners = new Set();

  get state() {
    return this.#state;
  }

  applyJson(json) {
    if (typeof json !== "string") {
      throw new TypeError("Host state must be a JSON string");
    }
    if (json.length > MAX_STATE_JSON_BYTES) {
      throw new RangeError("Host state exceeds the renderer size limit");
    }
    const previous = this.#state;
    const next = sanitizeState(JSON.parse(json), previous);
    this.#state = next;
    for (const listener of this.#listeners) listener(next, previous);
    return next;
  }

  subscribe(listener, immediate = false) {
    this.#listeners.add(listener);
    if (immediate) listener(this.#state, this.#state);
    return () => this.#listeners.delete(listener);
  }
}

const ACTIVATION_REQUIRED = new Set([
  "clear",
  "remove",
  "move",
  "swap",
  "openFiles",
  "savePlaylist",
  "restore",
]);

export class CommandGateway {
  constructor(request, hasActivation = () => false) {
    if (typeof request !== "function") {
      throw new TypeError("A host request function is required");
    }
    this.requestHost = request;
    this.hasActivation = hasActivation;
  }

  send(command, payload = null) {
    if (ACTIVATION_REQUIRED.has(command) && !this.hasActivation()) {
      return false;
    }
    this.requestHost(command, JSON.stringify(payload));
    return true;
  }
}

export function normalizeArchivePath(path) {
  if (typeof path !== "string") throw new TypeError("Archive path is not text");
  const normalized = path.replaceAll("\\", "/").replace(/^\.\//, "");
  const segments = normalized.split("/");
  if (
    normalized.startsWith("/") ||
    segments.some((segment) => segment === "..")
  ) {
    throw new Error(`Unsafe skin archive path: ${path}`);
  }
  return segments.filter((segment) => segment !== "").join("/");
}

export function findSkinPrefix(paths) {
  const skinXmlPaths = paths
    .map(normalizeArchivePath)
    .filter((path) => path.toLowerCase().endsWith("skin.xml"))
    .filter((path) => path.toLowerCase() === "skin.xml" || path.at(-9) === "/");
  if (skinXmlPaths.length !== 1) {
    throw new Error(
      `Modern skin archive must contain exactly one skin.xml; found ${skinXmlPaths.length}`,
    );
  }
  return skinXmlPaths[0].slice(0, -"skin.xml".length);
}

export function moveTargetAfterRemoval(indices, target) {
  const destination = Math.max(0, Math.trunc(finite(target)));
  const selected = new Set(
    (Array.isArray(indices) ? indices : [])
      .map((index) => Math.trunc(finite(index, -1)))
      .filter((index) => index >= 0),
  );
  let removedBeforeDestination = 0;
  for (const index of selected) {
    if (index < destination) removedBeforeDestination += 1;
  }
  return Math.max(0, destination - removedBeforeDestination);
}

export const limits = Object.freeze({
  maxStateJsonBytes: MAX_STATE_JSON_BYTES,
  maxTracks: MAX_TRACKS,
  maxVisualizationSamples: MAX_VIS_SAMPLES,
});
