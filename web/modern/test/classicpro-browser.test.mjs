import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import path from "node:path";
import esbuild from "esbuild";
import { adaptMakiText, adaptMakiSkinEngine, adaptMakiLayer } from "../maki-compat.mjs";

const directory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

class FakeElement {
  constructor(document, name) {
    this.ownerDocument = document;
    this.name = name;
    this.children = [];
    this.attributes = new Map();
    this.listeners = new Map();
    this.style = {
      setProperty(key, value) { this[key] = value; },
      removeProperty(key) { delete this[key]; },
    };
    this.classList = { add() {} };
  }
  append(...children) { this.children.push(...children); }
  appendChild(child) { this.append(child); return child; }
  replaceChildren(...children) { this.children = children; }
  setAttribute(key, value) { this.attributes.set(key, String(value)); }
  addEventListener(name, callback) { this.listeners.set(name, callback); }
  removeEventListener(name) { this.listeners.delete(name); }
  matches() { return false; }
  getBoundingClientRect() { return { left: 0, top: 0, width: 200, height: 100 }; }
}

class FakeDocument {
  constructor() {
    this.body = new FakeElement(this, "body");
    this.documentElement = { clientWidth: 800, clientHeight: 600 };
  }
  createElement(name) {
    const element = new FakeElement(this, name);
    if (name === "canvas") element.getContext = () => ({ measureText: text => ({ width: text.length * 7 }) });
    return element;
  }
  addEventListener() {}
  removeEventListener() {}
}

class FakeAudioNode {
  constructor() {
    this.frequency = { value: 0 };
    this.gain = { value: 0 };
    this.pan = { value: 0 };
  }
  connect(node) { return node; }
  getFloatTimeDomainData(values) { values.fill(0); }
}

class FakeAudioContext {
  constructor() { this.destination = new FakeAudioNode(); this.state = "running"; }
  createMediaElementSource() { return new FakeAudioNode(); }
  createGain() { return new FakeAudioNode(); }
  createAnalyser() { return new FakeAudioNode(); }
  createBiquadFilter() { return new FakeAudioNode(); }
}

function installBrowserGlobals(document) {
  const previousDocument = globalThis.document;
  const previousWindow = globalThis.window;
  const previousLocalStorage = globalThis.localStorage;
  const previousStereoPannerNode = globalThis.StereoPannerNode;
  const localStorage = { getItem() { return null; }, setItem() {}, removeItem() {} };
  globalThis.document = document;
  globalThis.window = { AudioContext: FakeAudioContext, document, localStorage, requestAnimationFrame() {} };
  globalThis.localStorage = localStorage;
  globalThis.StereoPannerNode = FakeAudioNode;
  return () => {
    globalThis.document = previousDocument;
    globalThis.window = previousWindow;
    globalThis.localStorage = previousLocalStorage;
    globalThis.StereoPannerNode = previousStereoPannerNode;
  };
}

async function loadFixture() {
  const result = await esbuild.build({
    plugins: [{ name: "maki-localized-text", setup(build) {
      build.onLoad({ filter: /[/\\]makiClasses[/\\]Layer\.ts$/ }, async args => ({
        contents: adaptMakiLayer(await readFile(args.path, "utf8")), loader: "ts",
      }));
      build.onLoad({ filter: /[/\\]skin[/\\]SkinEngine_WAL\.ts$/ }, async args => ({
        contents: adaptMakiSkinEngine(await readFile(args.path, "utf8")), loader: "ts",
      }));
      build.onLoad({ filter: /[/\\]makiClasses[/\\]Text\.ts$/ }, async args => ({
        contents: adaptMakiText(await readFile(args.path, "utf8"), path.join(directory, "src/maki-locales.js")), loader: "ts",
      }));
    } }],
    bundle: true,
    format: "esm",
    nodePaths: [path.join(directory, "node_modules")],
    platform: "node",
    write: false,
    stdin: {
      loader: "ts",
      resolveDir: directory,
      sourcefile: "classicpro-browser-fixture.ts",
      contents: `
        import { installClassicProBrowser, preferredLanguageId } from "./src/classicpro-browser.ts";
        import { makiInterface } from "./src/maki-members.js";
        import { installClassicProControls } from "./src/classicpro-controls.ts";
        import { installMakiActionEvents } from "./src/maki-events.ts";
        import Browser from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Browser.ts";
        import Button from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Button.ts";
        import Edit from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Edit.ts";
        import Text from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Text.ts";
        import Layer from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Layer.ts";
        import Group from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Group.ts";
        import SkinEngineWAL from "../../native/webamp/packages/webamp-modern/src/skin/SkinEngine_WAL.ts";
        import SystemObject from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/SystemObject.ts";
        import { XmlElement } from "@rgrove/parse-xml";
        export { makiInterface, installClassicProBrowser, installClassicProControls, installMakiActionEvents, preferredLanguageId, Browser, Button, Edit, Text, Layer, Group, SkinEngineWAL, SystemObject, XmlElement };
      `,
    },
  });
  return import(`data:text/javascript;base64,${Buffer.from(result.outputFiles[0].contents).toString("base64")}`);
}

test("ClassicPro Browser is materialized and reports policy-blocked navigation", async () => {
  const document = new FakeDocument();
  const restoreBrowser = installBrowserGlobals(document);
  try {
    const {
      Browser,
      Group,
      SkinEngineWAL,
      SystemObject,
      XmlElement,
      installClassicProBrowser,
    } = await loadFixture();
    installClassicProBrowser();

    const calls = [];
    let cancelNavigation = false;
    let heldUrl = "";
    let releaseBeforeNavigate = null;
    const root = {
      getImageManager() { return {}; },
      vm: {
        dispatch(_object, event, args = []) {
          calls.push({ event, args });
          if (event === "onbeforenavigate" && args[0]?.value === heldUrl) {
            return new Promise(resolve => { releaseBeforeNavigate = () => resolve(false); });
          }
          return event === "onbeforenavigate" && cancelNavigation;
        },
      },
    };
    const parent = new Group(root);
    const engine = new SkinEngineWAL(root);
    const browser = await engine.traverseChild(
      new XmlElement("browser", { id: "webbrowser", fitparent: "1", url: "https://example.invalid/" }),
      parent,
    );

    assert.ok(browser instanceof Browser);
    assert.equal(parent.getobject("webbrowser"), browser);
    browser.draw();
    browser.init();
    await new Promise(resolve => setImmediate(resolve));

    assert.equal(browser._div.style.position, "absolute");
    assert.equal(browser._div.style.overflow, "hidden");
    assert.equal(browser._div.children[0].attributes.get("data-kog-browser-error"), "policy-blocked");
    assert.match(browser._div.children[0].textContent, /unavailable in Kog's skin sandbox/);
    assert.deepEqual(calls.map(call => call.event), ["onbeforenavigate", "onnavigateerror"]);
    assert.equal(calls[1].args[0].value, "https://example.invalid/");
    assert.equal(calls[1].args[1].value, -1);

    browser._div.style.display = "none";
    browser.refresh();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(browser._div.style.display, "none", "browser rendering must preserve GuiObj visibility");

    // Reference Wasabi suppresses the IE error document without reporting a
    // successful navigation; Kog keeps the navigation-error event above.
    browser.setcancelieerrorpage("true");
    assert.equal(browser._div.children.length, 0, "native cancel-error-page flag suppresses the local error view");
    browser.setcancelieerrorpage("false");
    assert.equal(browser._div.children.length, 1);

    calls.length = 0;
    cancelNavigation = true;
    browser.navigateurl("https://cancelled.invalid/");
    await new Promise(resolve => setImmediate(resolve));
    assert.deepEqual(calls.map(call => call.event), ["onbeforenavigate"]);

    calls.length = 0;
    cancelNavigation = false;
    browser.navigateurl("https://stale.invalid/");
    browser.navigateurl("https://latest.invalid/");
    await new Promise(resolve => setImmediate(resolve));
    assert.deepEqual(calls.map(call => call.event), ["onbeforenavigate", "onnavigateerror"]);
    assert.equal(calls[0].args[0].value, "https://latest.invalid/");

    calls.length = 0;
    heldUrl = "https://in-flight.invalid/";
    browser.navigateurl(heldUrl);
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(typeof releaseBeforeNavigate, "function");
    browser.navigateurl("https://superseding.invalid/");
    await new Promise(resolve => setImmediate(resolve));
    releaseBeforeNavigate();
    await new Promise(resolve => setImmediate(resolve));
    assert.deepEqual(calls.map(call => [call.event, call.args[0]?.value]), [
      ["onbeforenavigate", "https://in-flight.invalid/"],
      ["onbeforenavigate", "https://superseding.invalid/"],
      ["onnavigateerror", "https://superseding.invalid/"],
    ]);
    heldUrl = "";

    browser.stop();
    assert.equal(browser._div.attributes.get("data-kog-browser-state"), "stopped");
    browser.navigateurl("https://second.invalid/");
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(calls.at(-1).event, "onnavigateerror");
    assert.equal(calls.at(-1).args[0].value, "https://second.invalid/");
    assert.equal(browser.getdocumenttitle(), "");
    assert.throws(() => browser.scrape(), /requires a loaded document/);

    assert.equal(typeof SystemObject.prototype.getlanguageid, "function");
    assert.equal(SystemObject.prototype.getbuildnumber, undefined, "do not invent a Winamp build number");

    calls.length = 0;
    browser.navigateurl("https://disposed.invalid/");
    browser.dispose();
    await new Promise(resolve => setImmediate(resolve));
    assert.deepEqual(calls, [], "dispose invalidates queued navigation callbacks");
  } finally {
    restoreBrowser();
  }
});

test("preferredLanguageId returns a canonical browser locale", async () => {
  const document = new FakeDocument();
  const restoreBrowser = installBrowserGlobals(document);
  try {
    const { preferredLanguageId } = await loadFixture();
    // Reference Wasabi getLanguageId() is a zero-argument UI locale query.
    assert.equal(preferredLanguageId({ language: "en-us", languages: ["fr-ca", "en-us"] }), "fr-ca");
    assert.equal(preferredLanguageId({ language: "not_a_locale", languages: [] }), "");
    assert.equal(preferredLanguageId({ language: "", languages: [] }), "");
  } finally {
    restoreBrowser();
  }
});

test("ClassicPro System strings come from loaded StringTable resources and URLs use Winamp encoding", async () => {
  const document = new FakeDocument();
  const restoreBrowser = installBrowserGlobals(document);
  try {
    const { SkinEngineWAL, SystemObject, XmlElement, installClassicProBrowser } = await loadFixture();
    installClassicProBrowser();
    const root = { getImageManager() { return {}; } };
    const engine = new SkinEngineWAL(root);
    const table = new XmlElement("StringTable", { id: "nullsoft.browser" });
    table.children.push(
      new XmlElement("StringEntry", { id: "17", string: "Location" }),
      new XmlElement("StringEntry", { id: "21", string: "Autoopen Media Monitor on media results" }),
      new XmlElement("StringEntry", { id: "invalid", string: "ignored" }),
    );
    await engine.traverseChild(table, null);
    const system = Object.create(SystemObject.prototype);
    system._uiRoot = root;

    assert.equal(system.getstring("nullsoft.browser", 17), "Location");
    assert.equal(system.getstring("nullsoft.browser", 21), "Autoopen Media Monitor on media results");
    assert.equal(system.getstring("NULLSOFT.BROWSER", 21), "", "native table names are exact");
    const supplement = new XmlElement("StringTable", { id: "nullsoft.browser" });
    supplement.children.push(new XmlElement("StringEntry", { id: "17", string: "Updated location" }));
    await engine.traverseChild(supplement, null);
    assert.equal(system.getstring("nullsoft.browser", 17), "Updated location");
    assert.equal(system.getstring("nullsoft.browser", 21), "Autoopen Media Monitor on media results",
      "later table fragments preserve unrelated entries");
    assert.equal(system.getstring("nullsoft.browser", 999), "");
    assert.equal(system.getstring("missing.table", 17), "");
    assert.equal(
      system.urlencode("AZaz09-_.~ !*'()/é"),
      "AZaz09-_.~%20%21%2A%27%28%29%2F%C3%A9",
    );
    assert.equal(system.urlencode(""), "");
    assert.equal(SystemObject.prototype.getbuildnumber, undefined, "do not invent a Winamp build number");
  } finally {
    restoreBrowser();
  }
});

test("modern text translates only presentation and measures the translated label", async () => {
  const document = new FakeDocument();
  const restoreBrowser = installBrowserGlobals(document);
  try {
    const { Text, Group, SkinEngineWAL, XmlElement, installMakiActionEvents, installClassicProBrowser } = await loadFixture();
    installMakiActionEvents();
    installClassicProBrowser();
    const root = {
      getImageManager() { return {}; }, vm: { dispatch() {} },
      async getFileAsString() { return '<StringTable><StringEntry id="7" string="Library"/></StringTable>'; },
    };
    const engine = new SkinEngineWAL(root);
    await engine.include(new XmlElement("include", { file: "strings.xml" }), null);
    const text = new Text(root);
    text.setXmlAttr("text", "@nullsoft.wasabi#7");
    assert.equal(text._textWrapper.innerText, "@nullsoft.wasabi#7");
    text.setXmlAttr("translate", "2");
    assert.equal(text._textWrapper.innerText, "Library");
    assert.equal(text.gettext(), "@nullsoft.wasabi#7", "MAKI still observes the source value");
    assert.equal(text.getautowidth(), 7 * 7, "TrueType layout geometry measures the translated text");
    assert.equal(text.gettextwidth(), "@nullsoft.wasabi#7".length * 7 + 4,
      "native script getTextWidth measures unlocalized text with its four-pixel padding");
    assert.equal(text._getBitmapFontTextWidth({ _charWidth: 6 }), 7 * 6);
    text.setalternatetext("@nullsoft.wasabi#7");
    assert.equal(text._textWrapper.innerText, "Library");
    text.setXmlAttr("translate", "0");
    assert.equal(text._textWrapper.innerText, "@nullsoft.wasabi#7");
    const control = new Group(root);
    control.setXmlAttr("translate", "2");
    control.setXmlAttr("tooltip", "@nullsoft.wasabi#7");
    assert.equal(control._div.attributes.get("title"), "Library");
    assert.equal(control._tooltip, "@nullsoft.wasabi#7");
    control.setXmlAttr("translate", "0");
    assert.equal(control._div.attributes.get("title"), "@nullsoft.wasabi#7");
    text.setalternatetext("");
    text._display = "time";
    text._displayValue = "  :  ";
    text.settext("12:34");
    assert.equal(text.gettext(), "12:34", "script clock text overrides the stopped display");
    text.settext("");
    assert.equal(text.gettext(), "  :  ", "clearing script text restores the dynamic display");
  } finally { restoreBrowser(); }
});

test("native Layer is movable by default and explicit move=0 disables dragging", async () => {
  const restoreBrowser = installBrowserGlobals(new FakeDocument());
  try {
    const { Layer } = await loadFixture();
    const layer = new Layer({ getImageManager() { return {}; } });
    assert.equal(layer._movable, true);
    assert.equal(layer._movingEventsRegistered, true);
    layer.setXmlAttr("move", "0");
    assert.equal(layer._movable, false);
    assert.equal(layer._movingEventsRegistered, false);
  } finally { restoreBrowser(); }
});

test("ClassicPro Browser materializes typed embedded XUI and DownloadsList controls", async () => {
  const document = new FakeDocument();
  const restoreBrowser = installBrowserGlobals(document);
  try {
    const {
      Button,
      Edit,
      Group,
      SkinEngineWAL,
      XmlElement,
      makiInterface,
      installClassicProBrowser,
      installClassicProControls,
      installMakiActionEvents,
    } = await loadFixture();
    installMakiActionEvents();
    installClassicProControls();
    installClassicProBrowser();
    const definitions = new Map();
    const xuiDefinitions = new Map();
    const history = new XmlElement("groupdef", {
      id: "wasabi.historyeditbox.main.group",
      embed_xui: "historyeditbox.edit",
    });
    history.children.push(new XmlElement("edit", { id: "historyeditbox.edit" }));
    definitions.set("wasabi.historyeditbox.main.group", history);
    const toggle = new XmlElement("groupdef", {
      id: "wasabi.togglebutton.group",
      xuitag: "Wasabi:ToggleButton",
      embed_xui: "wasabi.button",
    });
    toggle.children.push(new XmlElement("togglebutton", { id: "wasabi.button" }));
    definitions.set("wasabi.togglebutton.group", toggle);
    xuiDefinitions.set("wasabi:togglebutton", toggle);
    const channels = new XmlElement("groupdef", {
      id: "sc.channels",
      xuitag: "SC:Channels",
      embed_xui: "sc.main.ch",
    });
    channels.children.push(new XmlElement("layer", { id: "stereo" }));
    definitions.set("sc.channels", channels);
    xuiDefinitions.set("sc:channels", channels);
    const nav = new XmlElement("groupdef", { id: "browser.navurl" });
    nav.children.push(
      new XmlElement("Wasabi:HistoryEditBox", { id: "browser.hedit", text: "https://initial.invalid/" }),
      new XmlElement("Wasabi:Button", { id: "browser.navigate" }),
      new XmlElement("Wasabi:ToggleButton", { id: "browser.toggle" }),
      new XmlElement("SC:Channels", { id: "browser.channels" }),
    );
    definitions.set("browser.navurl", nav);
    const downloads = new XmlElement("groupdef", { id: "dlds.mode" });
    downloads.children.push(new XmlElement("DownloadsList", { id: "scraper.downloads" }));
    definitions.set("dlds.mode", downloads);
    const root = {
      getGroupDef(id) { return definitions.get(String(id).toLowerCase()) ?? null; },
      getImageManager() { return {}; },
      getXuiElement(tag) { return xuiDefinitions.get(String(tag).toLowerCase()) ?? null; },
      getBitmap() { return null; },
      hasBitmapFilepath() { return false; },
      addHeight() {},
      addWidth() {},
      vm: { dispatch() {} },
    };
    const engine = new SkinEngineWAL(root);
    const parent = new Group(root);
    const navGroup = await engine.traverseChild(new XmlElement("group", { id: "browser.navurl" }), parent);
    const historyWrapper = navGroup.findobject("browser.hedit");
    const historyEdit = makiInterface(historyWrapper, Edit);
    const navigateButton = navGroup.findobject("browser.navigate");
    const toggleWrapper = navGroup.findobject("browser.toggle");
    const toggleButton = makiInterface(toggleWrapper, Button);
    const channelGroup = navGroup.findobject("browser.channels");
    assert.ok(historyEdit instanceof Edit);
    assert.ok(navigateButton instanceof Button);
    assert.ok(toggleButton instanceof Button, "typed interface resolves the real inner object");
    assert.ok(channelGroup instanceof Group, "missing embed_xui targets fall back to their live wrapper");
    assert.ok(navGroup._children.includes(historyWrapper));
    assert.ok(navGroup._children.includes(toggleWrapper));
    assert.equal(historyWrapper.getId(), "browser.hedit");
    assert.equal(historyEdit.getId(), "historyeditbox.edit", "outer id must not rename the embedded control");
    assert.equal(historyEdit.gettext(), "https://initial.invalid/", "initial unknown XUI parameters reach the embedded Edit");
    assert.equal(makiInterface(historyWrapper, Group), historyWrapper);
    assert.equal(navGroup.getobject("browser.hedit"), historyWrapper, "direct Group lookup preserves wrapper identity");
    assert.equal(navGroup.getobject("browser.toggle"), toggleWrapper);
    assert.equal(navGroup.getobject("browser.channels"), channelGroup);
    assert.equal(navGroup.getobject("historyeditbox.edit"), null, "direct Group lookup does not expose unrelated descendants");
    historyEdit.settext("https://updated.invalid/");
    assert.equal(historyEdit.gettext(), "https://updated.invalid/");
    navigateButton.setactivated(true);
    assert.equal(navigateButton.getactivated(), true);
    toggleButton.setactivated(true);
    assert.equal(toggleButton.getactivated(), true);

    const downloadGroup = await engine.traverseChild(new XmlElement("group", { id: "dlds.mode" }), parent);
    const downloadList = downloadGroup.findobject("scraper.downloads");
    assert.ok(downloadList);
    downloadList.draw();
    assert.equal(downloadList._div.attributes.get("data-kog-downloads"), "unavailable");
    assert.match(downloadList._div.textContent, /blocks external content/);
    assert.equal(downloadList.handleAction("PLAY_SELECTED"), true);
    assert.match(downloadList._div.textContent, /PLAY_SELECTED.*unavailable/);
  } finally {
    restoreBrowser();
  }
});

test("ClassicPro Browser preserves the MAKI onBeforeNavigate Boolean result", async () => {
  const document = new FakeDocument();
  const restoreBrowser = installBrowserGlobals(document);
  try {
    const { Browser, Group, SkinEngineWAL, XmlElement, installClassicProBrowser } = await loadFixture();
    installClassicProBrowser();
    const variable = { type: "OBJECT", value: null };
    const script = {
      bindings: [{ methodOffset: 0, variableOffset: 0, commandOffset: 17 }],
      methods: [{ name: "onbeforenavigate" }],
      variables: [variable],
    };
    const interpreted = [];
    const root = {
      getImageManager() { return {}; },
      vm: {
        _scripts: [script],
        dispatch(_object, event) {
          assert.notEqual(event, "onbeforenavigate", "Boolean events bypass the listener-count dispatcher");
        },
        interpret(actualScript, offset, event, reversedArgs) {
          interpreted.push({ actualScript, offset, event, reversedArgs });
          return { type: "BOOLEAN", value: 1 };
        },
      },
    };
    const parent = new Group(root);
    const engine = new SkinEngineWAL(root);
    const browser = await engine.traverseChild(
      new XmlElement("browser", { id: "cancel-browser", url: "https://cancelled-by-maki.invalid/" }),
      parent,
    );
    assert.ok(browser instanceof Browser);
    variable.value = browser;
    browser.init();
    await new Promise(resolve => setImmediate(resolve));

    assert.equal(interpreted.length, 1);
    assert.equal(interpreted[0].actualScript, script);
    assert.equal(interpreted[0].offset, 17);
    assert.equal(interpreted[0].event, "onbeforenavigate");
    assert.equal(interpreted[0].reversedArgs.at(-1).value, "https://cancelled-by-maki.invalid/");
    assert.equal(browser._div.children.length, 0, "a true MAKI return cancels the blocked navigation");
  } finally {
    restoreBrowser();
  }
});

test("ClassicPro Browser bounds navigation bursts and history", async () => {
  const document = new FakeDocument();
  const restoreBrowser = installBrowserGlobals(document);
  const originalWarn = console.warn;
  const warnings = [];
  console.warn = message => warnings.push(String(message));
  try {
    const { Browser, Group, SkinEngineWAL, XmlElement, installClassicProBrowser } = await loadFixture();
    installClassicProBrowser();
    const calls = [];
    const root = {
      getImageManager() { return {}; },
      vm: {
        dispatch(_object, event, args = []) {
          calls.push({ event, args });
          return false;
        },
      },
    };
    const parent = new Group(root);
    const engine = new SkinEngineWAL(root);
    const browser = await engine.traverseChild(
      new XmlElement("browser", { id: "bounded-browser" }),
      parent,
    );
    assert.ok(browser instanceof Browser);
    browser.init();

    for (let index = 0; index < 128; index++) {
      browser.navigateurl(`https://bounded.invalid/${index}`);
    }
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(browser._div.attributes.get("data-kog-browser-state"), "blocked");
    assert.equal(browser._div.attributes.get("data-kog-browser-history-size"), "128");
    assert.equal(calls.at(-1).args[0].value, "https://bounded.invalid/127");

    const callCountAtBoundary = calls.length;
    browser.navigateurl("https://bounded.invalid/overflow");
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(calls.length, callCountAtBoundary, "overflow is cancelled before executing another callback");
    assert.equal(browser._div.attributes.get("data-kog-browser-state"), "limited");
    assert.equal(browser._div.attributes.get("data-kog-browser-history-size"), "128");
    assert.equal(browser._div.children[0].attributes.get("data-kog-browser-error"), "navigation-limit");
    assert.match(browser._div.children[0].textContent, /128 requests per second/);
    assert.deepEqual(warnings, ["ClassicPro Browser navigation limit exceeded (128 requests per second)."]);
  } finally {
    console.warn = originalWarn;
    restoreBrowser();
  }
});
