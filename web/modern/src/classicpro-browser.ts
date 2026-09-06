import Browser from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Browser";
import GuiObj from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/GuiObj";
import Group from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Group";
import SkinEngineWAL from "../../../native/webamp/packages/webamp-modern/src/skin/SkinEngine_WAL";
import SystemObject from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/SystemObject";
import XuiElement from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/XuiElement";
import { XmlElement } from "@rgrove/parse-xml";
import { getMakiEmbeddedObject, setMakiEmbeddedObject } from "./maki-members.js";

type MakiValue = { type: "INT" | "STRING"; value: number | string };

type BrowserLike = Browser & {
  _div: HTMLElement;
  _uiRoot?: {
    vm?: { dispatch?: (object: unknown, event: string, args?: MakiValue[]) => unknown };
  };
};

type BrowserState = {
  burstCount: number;
  burstStartedAt: number;
  cancelErrorPage: boolean;
  diagnostic: string;
  currentUrl: string;
  disposed: boolean;
  errorVisible: boolean;
  generation: number;
  history: string[];
  historyIndex: number;
  homeUrl: string;
  initialized: boolean;
  stopped: boolean;
  targetName: string;
};

const states = new WeakMap<object, BrowserState>();
const stringTables = new WeakMap<object, Map<string, Map<number, string>>>();
const POLICY_ERROR = -1;
const MAX_HISTORY_ENTRIES = 128;
const MAX_NAVIGATIONS_PER_WINDOW = 128;
const NAVIGATION_WINDOW_MS = 1000;
let installed = false;

function registerStringTable(uiRoot: object, node: XmlElement): void {
  const tableId = String(node.attributes.id ?? "");
  if (!tableId) return;
  let tables = stringTables.get(uiRoot);
  if (!tables) {
    tables = new Map();
    stringTables.set(uiRoot, tables);
  }
  // LocalesManager::AddString merges entries into the exact named table;
  // later fragments replace only matching ids, not the complete table.
  const entries = tables.get(tableId) ?? new Map<number, string>();
  for (const child of node.children) {
    if (!(child instanceof XmlElement) || child.name.toLowerCase() !== "stringentry") continue;
    const rawId = String(child.attributes.id ?? "").trim();
    if (!/^-?\d+$/.test(rawId)) continue;
    const id = Number(rawId);
    if (!Number.isSafeInteger(id) || id < 0) continue;
    entries.set(id, String(child.attributes.string ?? ""));
  }
  tables.set(tableId, entries);
}

function getString(uiRoot: object | undefined, table: unknown, id: unknown): string {
  if (!uiRoot) return "";
  const entryId = Number(id);
  if (!Number.isSafeInteger(entryId) || entryId < 0) return "";
  return stringTables.get(uiRoot)?.get(String(table ?? ""))?.get(entryId) ?? "";
}

function urlEncode(value: unknown): string {
  // Winamp's AutoUrl converts the input to UTF-8, preserves RFC 3986's
  // unreserved ASCII set, and emits uppercase percent escapes byte-by-byte.
  const bytes = new TextEncoder().encode(String(value ?? ""));
  let encoded = "";
  for (const byte of bytes) {
    const unreserved = (byte >= 0x41 && byte <= 0x5a)
      || (byte >= 0x61 && byte <= 0x7a)
      || (byte >= 0x30 && byte <= 0x39)
      || byte === 0x2d || byte === 0x2e || byte === 0x5f || byte === 0x7e;
    encoded += unreserved ? String.fromCharCode(byte) : `%${byte.toString(16).toUpperCase().padStart(2, "0")}`;
  }
  return encoded;
}

class OfflineDownloadsList extends GuiObj {
  private message = "Browser downloads are unavailable because Kog blocks external content in the skin sandbox.";

  private renderMessage(): void {
    this._div.setAttribute("data-kog-downloads", "unavailable");
    this._div.setAttribute("role", "status");
    this._div.style.position = "absolute";
    this._div.style.boxSizing = "border-box";
    this._div.style.overflow = "auto";
    this._div.textContent = this.message;
  }

  draw(): void {
    super.draw();
    this.renderMessage();
  }

  handleAction(action: string): boolean {
    const boundedAction = String(action ?? "").slice(0, 128);
    this.message = `DownloadsList action '${boundedAction}' is unavailable because Kog does not download external browser content.`;
    this.renderMessage();
    return true;
  }
}

async function materializeEmbeddedXui(
  engine: any,
  node: XmlElement,
  parent: unknown,
  definition: XmlElement,
): Promise<GuiObj> {
  const definitionId = definition.attributes.id;
  const embeddedId = definition.attributes.embed_xui;
  if (!definitionId || !embeddedId) {
    throw new Error(`ClassicPro XUI '${node.name}' requires a groupdef with id and embed_xui.`);
  }
  const wrapper = await engine.newGroup(
    XuiElement,
    new XmlElement("group", { id: definitionId }),
    parent,
  ) as XuiElement;
  // The direct child retains the instance identity. MAKI typed assignment and
  // interface dispatch, not untyped object lookup, select the embedded control.
  wrapper.setXmlAttributes(node.attributes);
  return wrapper;
}

function state(browser: object): BrowserState {
  const prior = states.get(browser);
  if (prior) return prior;
  const created = {
    burstCount: 0,
    burstStartedAt: -1,
    cancelErrorPage: false,
    diagnostic: "",
    currentUrl: "",
    disposed: false,
    errorVisible: false,
    generation: 0,
    history: [],
    historyIndex: -1,
    homeUrl: "about:blank",
    initialized: false,
    stopped: false,
    targetName: "",
  };
  states.set(browser, created);
  return created;
}

function bool(value: unknown): boolean {
  if (value === true) return true;
  if (value === false || value == null) return false;
  const text = String(value).trim().toLowerCase();
  if (text === "true") return true;
  if (text === "false" || text === "") return false;
  const numeric = Number(text);
  return Number.isFinite(numeric) && numeric !== 0;
}

function styleBrowser(browser: BrowserLike): void {
  const host = browser._div;
  host.style.position = "absolute";
  host.style.boxSizing = "border-box";
  host.style.overflow = "hidden";
}

function render(browser: BrowserLike): void {
  styleBrowser(browser);
  const current = state(browser);
  browser._div.setAttribute(
    "data-kog-browser-state",
    current.diagnostic ? "limited" : current.stopped ? "stopped" : "blocked",
  );
  browser._div.setAttribute("data-kog-browser-history-size", String(current.history.length));
  browser._div.replaceChildren();
  const policyError = current.errorVisible && !current.cancelErrorPage
    && current.currentUrl && current.currentUrl !== "about:blank";
  if (!current.diagnostic && !policyError) return;

  const message = browser._div.ownerDocument.createElement("div");
  message.setAttribute("data-kog-browser-error", current.diagnostic ? "navigation-limit" : "policy-blocked");
  message.setAttribute("role", current.diagnostic ? "alert" : "status");
  message.style.boxSizing = "border-box";
  message.style.width = "100%";
  message.style.height = "100%";
  message.style.padding = "1em";
  message.style.overflow = "auto";
  message.style.background = "var(--color-wasabi-window-background, #20242a)";
  message.style.color = "var(--color-wasabi-window-text, #e5e7eb)";
  message.textContent = current.diagnostic
    || "External browser content is unavailable in Kog's skin sandbox.";
  browser._div.append(message);
}

function allowNavigation(browser: BrowserLike): boolean {
  const current = state(browser);
  const now = globalThis.performance?.now?.() ?? Date.now();
  if (current.burstStartedAt < 0 || now - current.burstStartedAt >= NAVIGATION_WINDOW_MS) {
    current.burstStartedAt = now;
    current.burstCount = 0;
    current.diagnostic = "";
  }
  current.burstCount++;
  if (current.burstCount <= MAX_NAVIGATIONS_PER_WINDOW) return true;
  if (!current.diagnostic) {
    current.diagnostic = `ClassicPro Browser navigation limit exceeded (${MAX_NAVIGATIONS_PER_WINDOW} requests per second).`;
    console.warn(current.diagnostic);
  }
  return false;
}

function reserveNavigation(browser: BrowserLike, url: string): boolean {
  if (!url || url === "about:blank") return true;
  if (allowNavigation(browser)) return true;
  const current = state(browser);
  current.generation++;
  current.errorVisible = false;
  render(browser);
  return false;
}

function dispatch(browser: BrowserLike, event: string, args: MakiValue[]): unknown {
  const send = browser._uiRoot?.vm?.dispatch;
  if (typeof send === "function") return send.call(browser._uiRoot!.vm, browser, event, args);
  return undefined;
}

async function beforeNavigate(browser: BrowserLike, args: MakiValue[]): Promise<boolean> {
  const vm = browser._uiRoot?.vm as any;
  if (!vm) return false;

  // The generic dispatcher returns a listener count. This event instead has a
  // Boolean result, so preserve each real MAKI callback's return value.
  if (Array.isArray(vm._scripts) && typeof vm.interpret === "function") {
    let cancelled = false;
    for (const script of vm._scripts) {
      for (const binding of script.bindings ?? []) {
        if (script.methods?.[binding.methodOffset]?.name?.toLowerCase() !== "onbeforenavigate") continue;
        const variable = script.variables?.[binding.variableOffset];
        const matches = variable?.isClass
          ? variable.members?.some((index: number) => script.variables[index]?.value === browser)
          : variable?.type === "OBJECT" && variable.value === browser;
        if (!matches) continue;
        if (variable.isClass) variable.value = browser;
        const returned = await vm.interpret(script, binding.commandOffset, "onbeforenavigate", [...args].reverse());
        if (bool(returned?.value)) cancelled = true;
      }
    }
    return cancelled;
  }

  // Lightweight embedders may expose only dispatch; allow them to implement
  // the same Boolean contract directly.
  return bool(await dispatch(browser, "onbeforenavigate", args));
}

async function completeBlockedNavigation(
  browser: BrowserLike,
  generation: number,
  url: string,
  targetName: string,
): Promise<void> {
  const current = state(browser);
  const args: MakiValue[] = [
    { type: "STRING", value: url },
    { type: "INT", value: 0 },
    { type: "STRING", value: targetName },
  ];
  const cancelled = await beforeNavigate(browser, args);
  if (current.disposed || current.stopped || current.generation !== generation) return;
  if (cancelled) return;
  current.errorVisible = true;
  render(browser);
  dispatch(browser, "onnavigateerror", [
    { type: "STRING", value: url },
    { type: "INT", value: POLICY_ERROR },
  ]);
}

function publishBlockedNavigation(browser: BrowserLike): void {
  const current = state(browser);
  const generation = ++current.generation;
  current.errorVisible = false;
  if (current.disposed || !current.initialized || !current.currentUrl || current.currentUrl === "about:blank") {
    render(browser);
    return;
  }
  render(browser);
  const url = current.currentUrl;
  const targetName = current.targetName;
  queueMicrotask(() => {
    if (current.disposed || current.stopped || current.generation !== generation) return;
    void completeBlockedNavigation(browser, generation, url, targetName).catch(error => {
      if (!current.disposed && current.generation === generation) {
        console.warn("ClassicPro Browser navigation callback failed", error);
      }
    });
  });
}

function showHistoryEntry(browser: BrowserLike): void {
  const current = state(browser);
  current.currentUrl = current.history[current.historyIndex] ?? "";
  current.stopped = false;
  render(browser);
  publishBlockedNavigation(browser);
}

function navigateUrl(this: BrowserLike, url: string): void {
  const current = state(this);
  if (current.disposed) return;
  const requested = String(url ?? "").trim();
  const nextUrl = requested || "about:blank";
  if (!reserveNavigation(this, nextUrl)) return;
  current.currentUrl = nextUrl;
  current.stopped = false;
  current.history.splice(current.historyIndex + 1);
  current.history.push(current.currentUrl);
  current.historyIndex = current.history.length - 1;
  if (current.history.length > MAX_HISTORY_ENTRIES) {
    const discarded = current.history.length - MAX_HISTORY_ENTRIES;
    current.history.splice(0, discarded);
    current.historyIndex -= discarded;
  }
  publishBlockedNavigation(this);
}

function browserBack(this: BrowserLike): void {
  const current = state(this);
  if (current.historyIndex <= 0) return;
  if (!reserveNavigation(this, current.history[current.historyIndex - 1] ?? "")) return;
  current.historyIndex--;
  showHistoryEntry(this);
}

function browserForward(this: BrowserLike): void {
  const current = state(this);
  if (current.historyIndex + 1 >= current.history.length) return;
  if (!reserveNavigation(this, current.history[current.historyIndex + 1] ?? "")) return;
  current.historyIndex++;
  showHistoryEntry(this);
}

function browserStop(this: BrowserLike): void {
  const current = state(this);
  current.stopped = true;
  current.generation++;
  render(this);
}

function browserRefresh(this: BrowserLike): void {
  const current = state(this);
  if (current.disposed) return;
  if (!reserveNavigation(this, current.currentUrl)) return;
  current.stopped = false;
  render(this);
  publishBlockedNavigation(this);
}

function browserHome(this: BrowserLike): void {
  navigateUrl.call(this, state(this).homeUrl);
}

function setTargetName(this: BrowserLike, targetName: string): void {
  const current = state(this);
  current.targetName = String(targetName ?? "");
  this._div.setAttribute("data-kog-browser-target", current.targetName);
}

function setCancelErrorPage(this: BrowserLike, cancel: boolean): void {
  // Wasabi reference behavior: this flag suppresses IE's error document; it
  // does not turn a failed navigation into a successful one.
  const current = state(this);
  if (current.disposed) return;
  current.cancelErrorPage = bool(cancel);
  render(this);
}

function getDocumentTitle(this: BrowserLike): string {
  // No document is created when navigation is denied by policy.
  return "";
}

function scrape(this: BrowserLike): never {
  throw new Error("ClassicPro Browser.scrape requires a loaded document; external documents are blocked by Kog's skin sandbox.");
}

export function preferredLanguageId(
  source: Pick<Navigator, "language" | "languages"> | undefined = globalThis.navigator,
): string {
  const candidate = source?.languages?.find(value => typeof value === "string" && value.trim())
    ?? source?.language
    ?? "";
  if (!candidate) return "";
  try {
    return (Intl.getCanonicalLocales(candidate)[0] ?? "").toLowerCase();
  } catch {
    return "";
  }
}

/**
 * Installs the Wasabi Browser control as an explicitly offline view. Kog does
 * not grant skin code network or filesystem access: every non-blank navigation
 * produces an onNavigateError and a bounded in-control error page. The native
 * setCancelIEErrorPage flag suppresses that page without pretending the
 * navigation succeeded.
 */
export function installClassicProBrowser(): void {
  if (installed) return;
  installed = true;

  const originalGuiSetXmlAttr = GuiObj.prototype.setXmlAttr;
  GuiObj.prototype.setXmlAttr = function (key: string, value: string): boolean {
    if (key.toLowerCase() === "embed_xui") {
      setMakiEmbeddedObject(this, value);
      return true;
    }
    return originalGuiSetXmlAttr.call(this, key, value);
  };

  // EmbeddedXuiObject::onUnknownXuiParam forwards only parameters the wrapper
  // does not implement. Geometry/id stay on the wrapper; text reaches its Edit.
  const originalGroupSetXmlAttr = Group.prototype.setXmlAttr;
  Group.prototype.setXmlAttr = function (key: string, value: string): boolean {
    if (originalGroupSetXmlAttr.call(this, key, value)) return true;
    const embedded = getMakiEmbeddedObject(this);
    return embedded ? embedded.setXmlAttr(key, value) : false;
  };
  const originalDynamicXuiElement = SkinEngineWAL.prototype.dynamicXuiElement;
  SkinEngineWAL.prototype.dynamicXuiElement = async function (node, parent) {
    const definition = this._uiRoot.getXuiElement(node.name);
    if (!definition?.attributes.embed_xui) {
      return originalDynamicXuiElement.call(this, node, parent);
    }
    return materializeEmbeddedXui(this, node, parent, definition);
  };

  const originalTraverseChild = SkinEngineWAL.prototype.traverseChild;
  SkinEngineWAL.prototype.traverseChild = async function (node, parent) {
    const tag = node.name?.toLowerCase();
    if (tag === "stringtable") {
      registerStringTable(this._uiRoot, node);
      return;
    }
    if (tag === "browser") return this.newGui(Browser, node, parent);
    if (tag === "downloadslist") return this.newGui(OfflineDownloadsList, node, parent);
    if (tag === "wasabi:historyeditbox") {
      const definition = this._uiRoot.getGroupDef("wasabi.historyeditbox.main.group");
      if (!definition) {
        throw new Error("ClassicPro Wasabi:HistoryEditBox requires wasabi.historyeditbox.main.group.");
      }
      return materializeEmbeddedXui(this, node, parent, definition);
    }
    return originalTraverseChild.call(this, node, parent);
  };

  const originalSetXmlAttr = Browser.prototype.setXmlAttr;
  Browser.prototype.setXmlAttr = function (key: string, value: string): boolean {
    if (originalSetXmlAttr.call(this, key, value)) return true;
    const current = state(this);
    switch (key.toLowerCase()) {
      case "url":
        current.homeUrl = String(value || "about:blank");
        current.currentUrl = current.homeUrl;
        return true;
      case "targetname":
        setTargetName.call(this, value);
        return true;
      case "mainmb":
      case "scrollbars":
      case "wantfocus":
        // These are retained as declarative state for the offline element.
        this._div.setAttribute(`data-wasabi-${key.toLowerCase()}`, String(value));
        return true;
      default:
        return false;
    }
  };

  const originalDraw = Browser.prototype.draw;
  Browser.prototype.draw = function () {
    originalDraw.call(this);
    render(this);
  };
  const originalInit = Browser.prototype.init;
  Browser.prototype.init = function () {
    originalInit.call(this);
    const current = state(this);
    current.initialized = true;
    if (!reserveNavigation(this, current.currentUrl)) return;
    if (current.currentUrl) {
      current.history = [current.currentUrl];
      current.historyIndex = 0;
    }
    publishBlockedNavigation(this);
  };
  const originalDispose = Browser.prototype.dispose;
  Browser.prototype.dispose = function () {
    const current = state(this);
    current.disposed = true;
    current.generation++;
    originalDispose.call(this);
  };

  const browser = Browser.prototype as unknown as Record<string, unknown>;
  browser.navigateurl = navigateUrl;
  browser.gotourl = navigateUrl;
  browser.back = browserBack;
  browser.forward = browserForward;
  browser.stop = browserStop;
  browser.refresh = browserRefresh;
  browser.home = browserHome;
  browser.settargetname = setTargetName;
  browser.setcancelieerrorpage = setCancelErrorPage;
  browser.getdocumenttitle = getDocumentTitle;
  browser.scrape = scrape;

  // Wasabi reference behavior: getLanguageId() takes no arguments and returns
  // the lowercase active UI locale id (for example en-us), not a numeric code.
  SystemObject.prototype.getlanguageid = function () { return preferredLanguageId(); };
  SystemObject.prototype.getstring = function (table: string, id: number) {
    return getString(this._uiRoot, table, id);
  };
  SystemObject.prototype.urlencode = function (value: string) { return urlEncode(value); };
}
