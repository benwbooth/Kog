import "../../../native/webamp/packages/webamp-modern/src/css/webamp.css";
import "./runtime.css";

import { UIRoot } from "../../../native/webamp/packages/webamp-modern/src/UIRoot";
import SkinEngineWAL from "../../../native/webamp/packages/webamp-modern/src/skin/SkinEngine_WAL";
import { ZipFileExtractor } from "../../../native/webamp/packages/webamp-modern/src/skin/FileExtractor";
import PlayListGui from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/PlayListGui";
import SystemObject from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/SystemObject";
import Timer from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Timer";
import Text from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Text";
import {
  registerPainter,
  VisPaintHandler,
} from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Vis";
import { registerAction } from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/menuWa5actions";
import standardFrameXml from "../../../native/webamp/packages/webamp-modern/assets/freeform/xml/wasabi/xml/xui/standardframe/standardframe.xml";
import standardFrameElementsXml from "../../../native/webamp/packages/webamp-modern/assets/freeform/xml/wasabi/xml/xui/standardframe/standardframe-elements.xml";
import wasabiTextXml from "../../../native/webamp/packages/webamp-modern/assets/freeform/xml/wasabi/xml/xui/text/text.xml";
import {
  CommandGateway,
  findSkinPrefix,
  moveTargetAfterRemoval,
  normalizeArchivePath,
  StateStore,
} from "./state-adapter.js";

declare global {
  interface Window {
    qt?: { webChannelTransport?: unknown };
    QWebChannel?: new (
      transport: unknown,
      ready: (channel: { objects: { kog?: KogBridge } }) => void,
    ) => unknown;
    kogModern?: {
      root: UIRoot;
      state: StateStore;
      commands: CommandGateway;
    };
  }
}

type Signal = { connect(callback: () => void): void };
type KogBridge = {
  stateJson: string;
  tracksJson?: string;
  skinUrl: string;
  stateJsonChanged?: Signal;
  tracksJsonChanged?: Signal;
  skinUrlChanged?: Signal;
  request(command: string, payloadJson: string): unknown;
};

type Track = {
  id: string;
  title: string;
  artist: string;
  album: string;
  duration: number;
};

const store = new StateStore();
let gateway: CommandGateway | null = null;
let trustedInputTurn = false;

for (const type of [
  "pointerdown",
  "mousedown",
  "touchstart",
  "keydown",
  "click",
  "dblclick",
  "contextmenu",
  "drop",
]) {
  window.addEventListener(
    type,
    (event) => {
      if (!event.isTrusted) return;
      trustedInputTurn = true;
      queueMicrotask(() => {
        trustedInputTurn = false;
      });
    },
    { capture: true, passive: true },
  );
}

function hasRecentUserActivation() {
  return Boolean(trustedInputTurn || navigator.userActivation?.isActive);
}

function errorText(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

function showFatal(message: string) {
  const status = document.getElementById("runtime-status");
  if (!status) return;
  status.hidden = false;
  status.classList.add("fatal");
  status.textContent = message;
}

function showLoading(message: string) {
  const status = document.getElementById("runtime-status");
  if (!status) return;
  status.hidden = false;
  status.classList.remove("fatal");
  status.textContent = message;
}

function hideLoading() {
  const status = document.getElementById("runtime-status");
  if (status) status.hidden = true;
}

function reportError(error: unknown) {
  const message = errorText(error).slice(0, 2_048);
  console.error(error);
  gateway?.send("error", message);
  return message;
}

function connectWebChannel(): Promise<KogBridge> {
  return new Promise((resolve, reject) => {
    if (!window.qt?.webChannelTransport) {
      reject(new Error("Qt WebChannel transport is unavailable"));
      return;
    }
    if (!window.QWebChannel) {
      reject(new Error("Qt WebChannel runtime is unavailable"));
      return;
    }
    new window.QWebChannel(window.qt.webChannelTransport, (channel) => {
      if (!channel.objects.kog) {
        reject(new Error("Kog WebChannel object is unavailable"));
        return;
      }
      resolve(channel.objects.kog);
    });
  });
}

class NormalizedZipFileExtractor extends ZipFileExtractor {
  prefix = "";
  getSkinDirectory: () => string;
  builtInXml = new Map([
    ["xml/xui/standardframe/standardframe.xml", standardFrameXml],
    ["xml/xui/standardframe/standardframe-elements.xml", standardFrameElementsXml],
    ["xml/xui/text/text.xml", wasabiTextXml],
  ]);

  constructor(getSkinDirectory: () => string) {
    super();
    this.getSkinDirectory = getSkinDirectory;
  }

  async prepare(skinPath: string, response: Response) {
    await super.prepare(skinPath, response);
    const paths = Object.values(this._zip.files)
      .filter((entry) => !entry.dir)
      .map((entry) => normalizeArchivePath(entry.name));
    this.prefix = findSkinPrefix(paths);
  }

  resolve(filePath: string) {
    const withoutDefault = filePath.replaceAll("@DEFAULTSKINPATH@", "");
    return this.prefix + normalizeArchivePath(withoutDefault);
  }

  async getFileAsString(filePath: string) {
    if (!filePath) return null;
    if (this.getSkinDirectory()?.startsWith("assets/freeform/xml/wasabi/")) {
      const builtIn = this.builtInXml.get(normalizeArchivePath(filePath));
      if (builtIn != null) return builtIn;
    }
    return super.getFileAsString(this.resolve(filePath));
  }

  async getFileAsBytes(filePath: string) {
    if (!filePath) return null;
    return super.getFileAsBytes(this.resolve(filePath));
  }

  async getFileAsBlob(filePath: string) {
    if (!filePath) return null;
    return super.getFileAsBlob(this.resolve(filePath));
  }
}

function playlistTitle(track: Track | undefined, index: number) {
  if (!track) return `Track ${index + 1}`;
  if (track.artist && track.title) return `${track.artist} - ${track.title}`;
  return track.title || track.artist || `Track ${index + 1}`;
}

function timeText(seconds: number) {
  const whole = Math.max(0, Math.round(Number(seconds) || 0));
  const minutes = Math.floor(whole / 60);
  return `${minutes}:${String(whole % 60).padStart(2, "0")}`;
}

class SafePlayListGui extends PlayListGui {
  refresh = () => {
    this._contentPanel.replaceChildren();
    const playlist = this._uiRoot.playlist as any;
    const currentTrack = playlist.getcurrentindex();
    const selected = new Set<number>(playlist._selection || []);

    for (let index = 0; index < playlist.getnumtracks(); index += 1) {
      const line = document.createElement("div");
      line.dataset.index = String(index);
      line.classList.toggle("current", index === currentTrack);
      line.classList.toggle("selected", selected.has(index));
      line.tabIndex = -1;
      line.draggable = true;

      line.addEventListener("click", (event) => {
        if (event.shiftKey && this._selectedIndex >= 0) {
          const first = Math.min(this._selectedIndex, index);
          const last = Math.max(this._selectedIndex, index);
          playlist._selection = Array.from(
            { length: last - first + 1 },
            (_, offset) => first + offset,
          );
        } else if (event.ctrlKey || event.metaKey) {
          const next = new Set<number>(playlist._selection || []);
          if (next.has(index)) next.delete(index);
          else next.add(index);
          playlist._selection = [...next].sort((a, b) => a - b);
        } else {
          playlist._selection = [index];
        }
        this._selectedIndex = index;
        this.refresh();
      });
      line.addEventListener("dblclick", () => {
        playlist.playtrack(index);
      });
      line.addEventListener("dragstart", (event) => {
        if (!playlist._selection?.includes(index)) playlist._selection = [index];
        event.dataTransfer?.setData("text/x-kog-playlist", "selection");
        if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
      });
      line.addEventListener("dragover", (event) => {
        if (Array.from(event.dataTransfer?.types || []).includes("text/x-kog-playlist")) {
          event.preventDefault();
          event.dataTransfer.dropEffect = "move";
        }
      });
      line.addEventListener("drop", (event) => {
        event.preventDefault();
        const indices = [...(playlist._selection || [])].sort((a, b) => a - b);
        const target = moveTargetAfterRemoval(indices, index);
        gateway?.send("move", { indices, target });
      });

      const title = document.createElement("span");
      title.textContent = `${index + 1}. ${playlist.gettitle(index)}`;
      const duration = document.createElement("span");
      duration.textContent = playlist.getlength(index);
      line.append(title, duration);
      this._contentPanel.appendChild(line);
    }
  };

  init() {
    super.init();
    this._div.addEventListener("keydown", (event) => {
      const playlist = this._uiRoot.playlist as any;
      if (event.key === "Delete" && playlist._selection?.length) {
        event.preventDefault();
        gateway?.send("remove", [...playlist._selection]);
      } else if (event.key === "Enter" && this._selectedIndex >= 0) {
        event.preventDefault();
        playlist.playtrack(this._selectedIndex);
      }
    });
  }
}

class KogSkinEngine extends SkinEngineWAL {
  private includeRequests = 0;
  private guiObjects = 0;

  async include(node: any, parent: any) {
    if (++this.includeRequests > 4096) throw new Error("Modern skin XML include limit exceeded");
    return super.include(node, parent);
  }

  async newGui<Type>(Type: any, node: any, parent: any): Promise<any> {
    if (++this.guiObjects > 50_000) throw new Error("Modern skin GUI object limit exceeded");
    const SafeType = Type === PlayListGui ? SafePlayListGui : Type;
    return super.newGui(SafeType, node, parent);
  }
}

class HostAnalyser {
  fftSize = 1_024;
  frequencyBinCount = 512;
  minDecibels = -100;
  maxDecibels = -30;
  smoothingTimeConstant = 0;

  sample(source: number[], index: number, targetLength: number) {
    if (!source.length) return 0;
    const sourceIndex = Math.min(
      source.length - 1,
      Math.floor((index / Math.max(1, targetLength - 1)) * source.length),
    );
    return Number(source[sourceIndex]) || 0;
  }

  getByteFrequencyData(target: Uint8Array) {
    const source = store.state.visualization.spectrum;
    for (let index = 0; index < target.length; index += 1) {
      const value = this.sample(source, index, target.length);
      target[index] = Math.round(Math.max(0, Math.min(255, value <= 1 ? value * 255 : value)));
    }
  }

  getByteTimeDomainData(target: Uint8Array) {
    const source = store.state.visualization.wave;
    for (let index = 0; index < target.length; index += 1) {
      const value = this.sample(source, index, target.length);
      const byte = value >= -1 && value <= 1 ? (value + 1) * 127.5 : value;
      target[index] = Math.round(Math.max(0, Math.min(255, byte)));
    }
  }

  getFloatTimeDomainData(target: Float32Array) {
    const source = store.state.visualization.wave;
    for (let index = 0; index < target.length; index += 1) {
      const value = this.sample(source, index, target.length);
      target[index] = value >= -1 && value <= 1 ? value : (value - 128) / 128;
    }
  }
}

class HostPcmVisualizer extends VisPaintHandler {
  context: CanvasRenderingContext2D | null = null;

  prepare() {
    this.context = this._vis._canvas.getContext("2d");
  }

  paintFrame() {
    if (!this.context) this.prepare();
    if (!this.context) return;
    const canvas = this.context.canvas;
    const values = store.state.visualization.spectrum;
    this.context.clearRect(0, 0, canvas.width, canvas.height);
    if (!values.length) return;
    const bars = Math.max(1, Math.min(values.length, Math.floor(canvas.width / 2)));
    const width = canvas.width / bars;
    this.context.fillStyle = "#63e66d";
    for (let index = 0; index < bars; index += 1) {
      const value = Number(values[Math.floor((index / bars) * values.length)]) || 0;
      const normalized = Math.max(0, Math.min(1, value > 1 ? value / 255 : value));
      const height = Math.max(1, normalized * canvas.height);
      this.context.fillRect(index * width, canvas.height - height, Math.max(1, width - 1), height);
    }
  }

  doAction(action: string) {
    if (action === "vis_f5") gateway?.send("openVisualizer");
    else reportError(`Visualization action ${action} is not supported by the Kog host`);
  }
}

function installAudioAdapter(root: UIRoot) {
  const audio = root.audio as any;
  const analyser = new HostAnalyser();
  const unsupported = new Set<string>();
  let pendingSeek = false;
  let ready = false;
  const publishPlayback = () => {
    const playback = store.state.playback;
    audio.trigger(playback === "playing" ? "play" : playback === "paused" ? "pause" : "stop");
    audio.trigger("statchanged");
  };
  const unsupportedOnce = (feature: string) => {
    if (unsupported.has(feature)) return;
    unsupported.add(feature);
    reportError(`${feature} is not supported by the Kog audio host`);
  };

  audio._audio.pause();
  audio._audio.removeAttribute("src");
  audio._audio.autoplay = false;
  audio.setAudioSource = () => unsupportedOnce("Direct media URLs");
  audio.play = () => gateway?.send("play");
  audio.pause = () => gateway?.send("pause");
  audio.stop = () => gateway?.send("stop");
  audio.seekTo = (seconds: number) => {
    pendingSeek = true;
    gateway?.send("seek", Number(seconds));
  };
  audio.seekToPercent = (percent: number) => {
    pendingSeek = true;
    gateway?.send("seek", store.state.duration * Number(percent));
  };
  audio.getCurrentTime = () =>
    audio._timeRemaining
      ? store.state.position - store.state.duration
      : store.state.position;
  audio.getCurrentTimePercent = () =>
    store.state.duration > 0 ? store.state.position / store.state.duration : 0;
  audio.getLength = () => store.state.duration;
  audio.getState = () => store.state.playback;
  audio.getVolume = () => store.state.volume;
  audio.setVolume = (volume: number) => gateway?.send("volume", Number(volume));
  audio.getEqEnabled = () => store.state.eqEnabled;
  audio.setEqEnabled = (enabled: boolean) => gateway?.send("eqEnabled", Boolean(enabled));
  audio.getEq = (kind: string) => {
    const gain =
      kind === "preamp"
        ? store.state.eqPreamp
        : store.state.eq[Math.max(0, Number(kind) - 1)] || 0;
    return (gain + 12) / 24;
  };
  audio.setEq = (kind: string, value: number) => {
    const gain = Number(value) * 24 - 12;
    if (kind === "preamp") gateway?.send("eqPreamp", gain);
    else gateway?.send("eqBand", { index: Number(kind) - 1, gain });
  };
  audio.getAnalyser = () => analyser;
  // AudioPlayer's animation loop captures this original node. Feed that loop
  // host PCM too, so it cannot overwrite the MAKI VU meter with browser silence.
  audio._analyser.getFloatTimeDomainData = analyser.getFloatTimeDomainData.bind(analyser);
  audio._analyser.getByteTimeDomainData = analyser.getByteTimeDomainData.bind(analyser);
  audio._analyser.getByteFrequencyData = analyser.getByteFrequencyData.bind(analyser);
  audio.onSeek = (callback: () => void) => audio.on("seek", callback);
  audio.getBalance = () => 0;
  audio.setBalance = () => unsupportedOnce("Balance control");
  audio.getPlaybackRate = () => 1;
  audio.setPlaybackRate = () => unsupportedOnce("Playback-rate control");

  store.subscribe((next: any, previous: any) => {
    audio._isStop = next.playback === "stopped";
    audio._eqEnabled = next.eqEnabled;
    if (next.position !== previous.position || next.duration !== previous.duration) {
      audio.trigger("timeupdate");
      if (pendingSeek) {
        pendingSeek = false;
        audio.trigger("seek");
      }
    }
    if (next.volume !== previous.volume) audio.trigger("volumechanged");
    const track = next.tracks[next.currentIndex];
    const oldTrack = previous.tracks[previous.currentIndex];
    const trackChanged = next.currentIndex !== previous.currentIndex ||
      ["id", "title", "artist", "album", "duration"].some(key => track?.[key] !== oldTrack?.[key]);
    if (ready && (next.playback !== previous.playback || trackChanged)) {
      // Playlist subscribers must install the new metadata before MAKI callbacks.
      queueMicrotask(publishPlayback);
    }
    if (next.eqEnabled !== previous.eqEnabled) audio.trigger("statchanged");
    for (let index = 0; index < 10; index += 1) {
      if (next.eq[index] !== previous.eq[index]) audio._eqEmitter.trigger(String(index + 1));
    }
    if (next.eqPreamp !== previous.eqPreamp) audio._eqEmitter.trigger("preamp");
    if (next.visualization.wave !== previous.visualization.wave) {
      const wave = next.visualization.wave;
      const squareSum = wave.reduce((sum: number, value: number) => {
        const sample = value >= -1 && value <= 1 ? value : (value - 128) / 128;
        return sum + sample * sample;
      }, 0);
      audio._vuMeter = wave.length ? Math.sqrt(squareSum / wave.length) : 0;
    }
  }, true);
  return () => {
    ready = true;
    root.playlist.trigger("trackchange");
    publishPlayback();
    audio.trigger("timeupdate");
    audio.trigger("volumechanged");
  };
}

function installPlaylistAdapter(root: UIRoot) {
  const playlist = root.playlist as any;
  let syncingConfig = false;
  let firstSync = true;

  playlist.addTrack = () => gateway?.send("openFiles");
  playlist.enqueuefile = () => gateway?.send("openFiles");
  playlist.clear = () => gateway?.send("clear");
  playlist.removetrack = (item: number) => gateway?.send("remove", [Number(item)]);
  playlist.swaptracks = (item1: number, item2: number) =>
    gateway?.send("swap", { first: Number(item1), second: Number(item2) });
  playlist.moveup = (item: number) =>
    gateway?.send("move", { indices: [Number(item)], target: Math.max(0, Number(item) - 1) });
  playlist.movedown = (item: number) =>
    gateway?.send("move", { indices: [Number(item)], target: Number(item) + 1 });
  playlist.moveto = (item: number, target: number) =>
    gateway?.send("move", { indices: [Number(item)], target: Number(target) });
  playlist.playtrack = (item: number) => gateway?.send("playIndex", Number(item));
  playlist.gettitle = (item: number) => playlistTitle(store.state.tracks[item], item);
  playlist.getlength = (item: number) => timeText(store.state.tracks[item]?.duration || 0);
  playlist.getfilename = (item: number) => store.state.tracks[item]?.title || "";
  playlist.currentTrack = () => playlist._tracks[playlist._currentIndex] || null;
  playlist.showtrack = (item: number) => {
    const line = document.querySelector(`[data-index="${Number(item)}"]`);
    line?.scrollIntoView({ block: "nearest" });
  };
  playlist.showcurrentlyplayingtrack = () => playlist.showtrack(playlist._currentIndex);

  playlist._shuffleAttrib.on("datachanged", () => {
    if (!syncingConfig) {
      gateway?.send("shuffle", playlist._shuffleAttrib.getdata() === "0" ? "off" : "all");
    }
  });
  playlist._repeatAttrib.on("datachanged", () => {
    if (syncingConfig) return;
    const value = Number(playlist._repeatAttrib.getdata());
    gateway?.send("repeat", value < 0 ? "track" : value > 0 ? "playlist" : "off");
  });

  store.subscribe((next: any, previous: any) => {
    if (firstSync || next.revision !== previous.revision || next.tracks !== previous.tracks) {
      playlist._tracks = next.tracks.map((track: Track, index: number) => ({
        id: index + 1,
        filename: track.title || `Track ${index + 1}`,
        title: track.title,
        duration: track.duration,
        metadata: {
          title: track.title,
          artist: track.artist,
          album: track.album,
        },
      }));
      playlist._trackCounter = playlist._tracks.length + 1;
      playlist._selection = playlist._selection.filter(
        (index: number) => index >= 0 && index < playlist._tracks.length,
      );
    }
    playlist._currentIndex = next.currentIndex;
    if (
      firstSync ||
      next.revision !== previous.revision ||
      next.currentIndex !== previous.currentIndex ||
      next.tracks !== previous.tracks
    ) {
      playlist.trigger("trackchange");
    }

    syncingConfig = true;
    try {
      const shuffle = next.shuffle === "off" ? "0" : "1";
      const repeat = next.repeat === "track" ? "-1" : next.repeat === "playlist" ? "1" : "0";
      if (playlist._shuffleAttrib.getdata() !== shuffle) playlist._shuffleAttrib.setdata(shuffle);
      if (playlist._repeatAttrib.getdata() !== repeat) playlist._repeatAttrib.setdata(repeat);
    } finally {
      syncingConfig = false;
      firstSync = false;
    }
  }, true);
}

function installUiActions(root: UIRoot) {
  root.next = () => {
    gateway?.send("next");
  };
  root.previous = () => {
    gateway?.send("previous");
  };
  root.eject = () => {
    gateway?.send("openFiles");
  };
  root.eq_toggle = () => {
    gateway?.send("eqEnabled", !store.state.eqEnabled);
  };

  const action = (command: string, payload: unknown = null) => ({
    onExecute: () => {
      gateway?.send(command, payload);
      return true;
    },
  });
  for (const id of [1032, 1036, 40029, 40145]) registerAction(id, action("openFiles"));
  registerAction(40202, action("restore"));
  registerAction(40204, action("savePlaylist"));
  registerAction(40219, action("openGallery"));
  for (const id of [40191, 40192, 40221]) registerAction(id, action("openVisualizer"));
  for (const id of [40172, 40173, 40174, 40175, 40176, 40177, 40178, 40180, 40253, 40254]) {
    registerAction(id, action("openEqualizer"));
  }

  const systemPrototype = SystemObject.prototype as any;
  const currentMetadata = (system: any) => {
    const track = system._uiRoot.playlist.currentTrack();
    return track?.metadata || {};
  };
  systemPrototype.getplayitemstring = function getPlayItemString() {
    return this._uiRoot.playlist.getCurrentTrackTitle();
  };
  systemPrototype.getplayitemdisplaytitle = function getPlayItemDisplayTitle() {
    return this._uiRoot.playlist.getCurrentTrackTitle();
  };
  systemPrototype.getplayitemmetadatastring = function getPlayItemMetadataString(name: string) {
    const normalized = String(name).toLowerCase();
    if (normalized === "length") return String(store.state.duration);
    const value = currentMetadata(this)[normalized];
    return value == null ? "" : String(value);
  };
  systemPrototype.getmetadatastring = function getMetadataString(_filename: string, name: string) {
    return this.getplayitemmetadatastring(name);
  };
  systemPrototype.getsonginfotext = () => store.state.songInfo;
  systemPrototype.geteq = () => Number(store.state.eqEnabled);
  systemPrototype.seteq = (_enabled: number) => gateway?.send("eqEnabled", Boolean(_enabled));
  systemPrototype.geteqpreamp = () => Math.round((store.state.eqPreamp / 12) * 127);

  const timerSetDelay = Timer.prototype.setdelay;
  Timer.prototype.setdelay = function boundedTimerDelay(milliseconds: number) {
    return timerSetDelay.call(this, Math.max(16, Math.min(3_600_000, Number(milliseconds) || 16)));
  };

  const textGetText = Text.prototype.gettext;
  const textSetText = Text.prototype.settext;
  Text.prototype.settext = function hostSetText(value: string) {
    // Winamp 3 skins (including MMD3) end temporary ticker messages with
    // setText(""). Clear the alternate override even when _text is already "".
    if (value === "" && (this as any)._alternateText) {
      (this as any)._alternateText = "";
      textSetText.call(this, value);
      this._renderText();
      return;
    }
    textSetText.call(this, value);
  };
  Text.prototype.gettext = function hostMetadataText() {
    if ((this as any)._display === "songinfo") return store.state.songInfo;
    return textGetText.call(this);
  };
}

async function applyHostState(bridge: KogBridge) {
  try {
    store.applyJson(bridge.stateJson || "{}");
  } catch (error) {
    reportError(new Error(`Invalid Kog host state: ${errorText(error)}`));
  }
}

function applyHostTracks(bridge: KogBridge) {
  if (typeof bridge.tracksJson !== "string") return;
  try {
    // Separate durable property avoids losing the rows when Qt coalesces
    // high-frequency transport state notifications.
    store.applyJson(`{"tracks":${bridge.tracksJson}}`);
  } catch (error) {
    reportError(new Error(`Invalid Kog playlist state: ${errorText(error)}`));
  }
}

async function loadSkin(root: UIRoot, skinUrl: string) {
  showLoading("Loading modern skin…");
  let response: Response;
  try {
    response = await fetch(skinUrl, {
      cache: "no-store",
      credentials: "omit",
      redirect: "error",
    });
  } catch (_) {
    // Qt 6.4 has CORS-enabled custom-scheme GETs, but FetchApiAllowed was
    // introduced in Qt 6.6. Keep Ubuntu's older Qt working with a bodyless GET.
    // The native interceptor/handler restrict both paths to the same archive.
    const data = await new Promise<ArrayBuffer>((resolve, reject) => {
      const request = new XMLHttpRequest();
      request.open("GET", skinUrl);
      request.responseType = "arraybuffer";
      request.timeout = 30_000;
      request.onload = () => {
        if ((request.status === 0 || request.status === 200) && request.response instanceof ArrayBuffer)
          resolve(request.response);
        else reject(new Error(`Modern skin request failed (${request.status})`));
      };
      request.onerror = () => reject(new Error("Unable to read the selected modern skin"));
      request.ontimeout = () => reject(new Error("Modern skin request timed out"));
      request.send();
    });
    response = new Response(data, { status: 200 });
  }
  if (!response.ok && response.status !== 0) {
    throw new Error(`Modern skin request failed (${response.status})`);
  }

  const extractor = new NormalizedZipFileExtractor(() => root.getSkinDir());
  await extractor.prepare(skinUrl, response);
  root.setSkinUrl(skinUrl);
  root.setFileExtractor(extractor as any);
  root.SkinEngineClass = KogSkinEngine;

  const engine = new KogSkinEngine(root);
  await engine.buildUI();
}

async function main() {
  showLoading("Connecting to Kog…");
  const bridge = await connectWebChannel();
  gateway = new CommandGateway(
    (command, payloadJson) => bridge.request(command, payloadJson),
    hasRecentUserActivation,
  );
  bridge.stateJsonChanged?.connect(() => void applyHostState(bridge));
  bridge.tracksJsonChanged?.connect(() => applyHostTracks(bridge));
  await applyHostState(bridge);
  applyHostTracks(bridge);

  const mount = document.getElementById("web-amp");
  if (!mount) throw new Error("Modern skin mount point is missing");
  const root = new UIRoot("ui-root");
  mount.appendChild(root.getRootDiv());

  registerPainter("milkdrop", HostPcmVisualizer as any);
  const publishInitialPlayback = installAudioAdapter(root);
  installPlaylistAdapter(root);
  installUiActions(root);
  root.on("onlogmessage", (message: string) => {
    if (message) showLoading(message);
  });

  const skinUrl = bridge.skinUrl;
  if (typeof skinUrl !== "string" || !skinUrl.startsWith("kogskin://current/skin.wal")) {
    throw new Error("Kog supplied an invalid modern skin URL");
  }
  await loadSkin(root, skinUrl);
  publishInitialPlayback();
  hideLoading();
  window.kogModern = { root, state: store, commands: gateway };
  gateway.send("ready");
}

window.addEventListener("error", (event) => reportError(event.error || event.message));
window.addEventListener("unhandledrejection", (event) => reportError(event.reason));

void main().catch((error) => {
  const message = reportError(error);
  showFatal(`Unable to load this modern skin: ${message}`);
});
