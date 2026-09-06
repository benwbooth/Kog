import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import path from "node:path";
import esbuild from "esbuild";

const directory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

class FakeElement {
  constructor(document, name) {
    this.ownerDocument = document;
    this.name = name;
    this.children = [];
    this.dataset = {};
    this.attributes = new Map();
    this.listeners = new Map();
    this.style = {
      setProperty(key, value) { this[key] = value; },
      removeProperty(key) { delete this[key]; },
    };
    this.classList = { add() {} };
    this.clientHeight = 20;
    this.clientWidth = 100;
    this.scrollHeight = 100;
    this.scrollTop = 0;
    this.textContent = "";
  }
  append(...children) { this.children.push(...children); }
  appendChild(child) { this.append(child); return child; }
  replaceChildren(...children) { this.children = children; }
  remove() { this.removed = true; }
  setAttribute(key, value) { this.attributes.set(key, String(value)); }
  getAttribute(key) { return this.attributes.get(key) ?? null; }
  addEventListener(name, callback) { this.listeners.set(name, callback); }
  emit(name, event = {}) { this.listeners.get(name)?.({ preventDefault() {}, ...event }); }
  focus() { this.focused = true; this.emit("focus"); }
  select() { this.selected = true; }
  scrollIntoView() { this.scrolled = true; }
  getBoundingClientRect() { return { x: 0, y: 0, left: 0, top: 0, width: this.clientWidth, height: this.clientHeight }; }
}

class FakeDocument {
  constructor() { this.body = new FakeElement(this, "body"); }
  createElement(name) { return new FakeElement(this, name); }
  addEventListener() {}
  removeEventListener() {}
}

class FakeStorage {
  #values = new Map();
  getItem(key) { return this.#values.get(String(key)) ?? null; }
  setItem(key, value) { this.#values.set(String(key), String(value)); }
  removeItem(key) { this.#values.delete(String(key)); }
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
  createAnalyser() { const node = new FakeAudioNode(); node.fftSize = 1024; return node; }
  createBiquadFilter() { return new FakeAudioNode(); }
}

function installBrowserGlobals(document) {
  const previousDocument = globalThis.document;
  const previousWindow = globalThis.window;
  const previousLocalStorage = globalThis.localStorage;
  const previousStereoPannerNode = globalThis.StereoPannerNode;
  globalThis.document = document;
  const localStorage = new FakeStorage();
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
    bundle: true,
    format: "esm",
    nodePaths: [path.join(directory, "node_modules")],
    platform: "node",
    write: false,
    stdin: {
      loader: "ts",
      resolveDir: directory,
      sourcefile: "classicpro-controls-fixture.ts",
      contents: `
        import { installClassicProControls } from "./src/classicpro-controls.ts";
        import Edit from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Edit.ts";
        import Group from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Group.ts";
        import GroupList from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/GroupList.ts";
        import GuiList from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/GuiList.ts";
        import SkinEngineWAL from "../../native/webamp/packages/webamp-modern/src/skin/SkinEngine_WAL.ts";
        import { XmlElement } from "@rgrove/parse-xml";
        export { installClassicProControls, Edit, Group, GroupList, GuiList, SkinEngineWAL, XmlElement };
      `,
    },
  });
  return import(`data:text/javascript;base64,${Buffer.from(result.outputFiles[0].contents).toString("base64")}`);
}

test("ClassicPro GuiList keeps safe row, selection, scroll, and VM event state", async () => {
  const document = new FakeDocument();
  const restoreBrowser = installBrowserGlobals(document);
  try {
    const { GuiList, installClassicProControls } = await loadFixture();
    installClassicProControls();
    const calls = [];
    const list = Object.create(GuiList.prototype);
    list._div = document.createElement("guilist");
    const bitmapCalls = [];
    list._uiRoot = {
      getBitmap(id) { return id === "safe-id" ? { setAsBackground(element) { bitmapCalls.push(element); } } : null; },
      vm: { dispatch(object, event, args) { assert.equal(this, list._uiRoot.vm); calls.push({ object, event, args }); } },
    };
    list.setXmlAttr("multiselect", "1");
    assert.equal(list.setfontsize(12), 12);
    assert.equal(list.additem("<not markup>"), 0);
    list.additem("second");
    list.setsubitem(1, 1, "detail");
    list.setitemicon(1, "safe-id");
    list.setshowicons(1);
    assert.equal(list.getitemlabel(1, 1), "detail");
    assert.equal(list._div.children[0].children.at(-1).textContent, "<not markup>");
    assert.equal(list._div.children[1].children[0].attributes.get("data-bitmap-id"), "safe-id");
    assert.equal(bitmapCalls.length, 1);
    list.setselected(0, 1);
    list.setselected(1, 1);
    assert.equal(list.getfirstitemselected(), 0);
    assert.equal(list.getnextitemselected(0), 1);
    list.scrolltoitem(1);
    assert.equal(list._div.children[1].scrolled, true);
    list._div.children[1].emit("dblclick");
    assert.equal(calls.at(-1).event, "ondoubleclick");
    assert.equal(calls.at(-1).args[0].value, 1);
    assert.equal(list.deletebypos(0), 1);
    assert.equal(list.getitemfocused(), 0);
    assert.equal(list.getitemlabel(0, 0), "second");
    list.deleteallitems();
    assert.equal(list.getnumitems(), 0);
  } finally {
    restoreBrowser();
  }
});

test("ClassicPro GuiList renders stateful column labels and honors header visibility XML", async () => {
  const document = new FakeDocument();
  const restoreBrowser = installBrowserGlobals(document);
  try {
    const { GuiList, installClassicProControls } = await loadFixture();
    installClassicProControls();
    const list = Object.create(GuiList.prototype);
    list._div = document.createElement("guilist");
    list._uiRoot = { vm: { dispatch() {} } };
    list.setXmlAttr("columnwidths", "270;-1");
    list.setXmlAttr("columnlabels", "Media From;");
    list.setXmlAttr("numcolumns", "2");
    list.additem("Fixture Artist - Fixture Title");
    list.setsubitem(0, 1, "https://example.invalid/item");
    list.setselected(0, 1);

    list.setcolumnlabel(1, "Location");
    const header = list._div.children[0];
    const row = list._div.children[1];
    assert.equal(header.attributes.get("data-classicpro-columns"), "1");
    assert.equal(header.children[0].textContent, "Media From");
    assert.equal(header.children[1].textContent, "Location");
    assert.equal(header.style.gridTemplateColumns, "270px minmax(0, 1fr)");
    assert.equal(row.attributes.get("data-classicpro-row"), "0");
    assert.equal(row.children[0].textContent, "Fixture Artist - Fixture Title");
    assert.equal(row.children[1].textContent, "https://example.invalid/item");
    assert.equal(row.attributes.get("aria-selected"), "true");
    assert.equal(list.getnumitems(), 1, "changing a column label preserves rows");
    assert.equal(list.getitemlabel(0, 1), "https://example.invalid/item");

    list.setXmlAttr("showcolumns", "0");
    assert.equal(list._div.children.length, 1);
    assert.equal(list._div.children[0].attributes.get("data-classicpro-row"), "0");
    assert.equal(list.getitemselected(0), 1, "hiding headers preserves selection");
    list.setXmlAttr("nocolheader", "0");
    assert.equal(list._div.children[0].attributes.get("data-classicpro-columns"), "1");
    list.setXmlAttr("nocolheader", "1");
    assert.equal(list._div.children[0].attributes.get("data-classicpro-row"), "0");
  } finally {
    restoreBrowser();
  }
});

test("ClassicPro Edit and GroupList turn script calls into DOM/controller state", async () => {
  const document = new FakeDocument();
  const restoreBrowser = installBrowserGlobals(document);
  try {
    const { Edit, Group, GroupList, SkinEngineWAL, XmlElement, installClassicProControls } = await loadFixture();
    installClassicProControls();
    const calls = [];
    const definitions = new Map();
    const root = {
      getImageManager() { return {}; },
      vm: { dispatch(_object, event) { calls.push(event); } },
      getGroupDef(id) { return definitions.get(id) ?? null; },
    };
    const definition = new XmlElement("groupdef", { id: "widgets.manager.listitem", h: "24" });
    definition.children.push(new XmlElement("text", { id: "row.title", text: "Nested title" }));
    const widgetDefinition = new XmlElement("groupdef", { id: "widget.dynamic" });
    widgetDefinition.children.push(new XmlElement("text", { id: "widget.title", text: "Dynamic title" }));
    const replacementDefinition = new XmlElement("groupdef", { id: "widget.replacement" });
    replacementDefinition.children.push(new XmlElement("text", { id: "widget.replacement.title", text: "Replacement title" }));
    definitions.set("widgets.manager.listitem", definition);
    definitions.set("widget.dynamic", widgetDefinition);
    definitions.set("widget.replacement", replacementDefinition);
    const edit = Object.create(Edit.prototype);
    edit._div = document.createElement("edit");
    edit._div.style.display = "none";
    edit._uiRoot = root;
    edit.settext("playlist search");
    assert.equal(edit.gettext(), "playlist search");
    assert.equal(edit._div.style.position, "absolute");
    assert.equal(edit._div.style.display, "none", "control rendering must preserve GuiObj visibility");
    assert.equal(edit._div.style.overflow, "hidden");
    assert.equal(edit._div.children[0].style.boxSizing, "border-box");
    assert.equal(edit._div.children[0].style.border, "0");
    assert.equal(edit._div.children[0].style.padding, "0");
    edit.setfocus();
    assert.equal(edit._div.children[0].focused, true);
    edit._div.children[0].value = "updated";
    edit._div.children[0].emit("input");
    edit._div.children[0].emit("keydown", { key: "Enter" });
    assert.equal(edit.gettext(), "updated");
    assert.deepEqual(calls, ["oneditupdate", "onenter"]);
    edit.setautoenter(true);
    edit._div.children[0].emit("blur");
    assert.equal(calls.at(-1), "onenter");
    await new Promise(resolve => setTimeout(resolve, 400));
    assert.equal(calls.at(-1), "onidleeditupdate");
    calls.length = 0;

    const groups = Object.create(GroupList.prototype);
    groups._div = document.createElement("grouplist");
    groups._uiRoot = root;
    groups._children = [];
    // Warm the installer cache with the actual SkinEngineWAL materializer—not
    // a test double—and prove it can resolve a nested groupdef child.
    const engine = new SkinEngineWAL(root);
    const detachedParent = new Group(root);
    const parsedGroupList = await engine.traverseChild(
      new XmlElement("grouplist", { id: "parsed.grouplist" }),
      detachedParent,
    );
    assert.ok(parsedGroupList instanceof GroupList);
    assert.equal(detachedParent.getobject("parsed.grouplist"), parsedGroupList);
    const customObject = await engine.traverseChild(
      new XmlElement("customobject", { id: "widget.holder", groupid: "widget.dynamic" }),
      detachedParent,
    );
    const initialWidget = customObject.getobject("widget.dynamic");
    assert.equal(initialWidget.findobject("widget.title").gettext(), "Dynamic title");
    // Script calls use the awaited setxmlparam path and replace the mounted
    // group rather than reporting success for an empty Group shell.
    await customObject.setxmlparam("groupid", "widget.replacement");
    assert.equal(customObject.getobject("widget.replacement").findobject("widget.replacement.title").gettext(), "Replacement title");
    await assert.rejects(
      customObject.setxmlparam("groupid", "widget.missing"),
      /cannot materialize unknown groupid: widget\.missing/,
    );
    assert.ok(customObject.getobject("widget.replacement"));
    const warmup = await engine.newGroup(Group, new XmlElement("group", { id: "widgets.manager.listitem" }), detachedParent);
    assert.equal(warmup.findobject("row.title").gettext(), "Nested title");

    const item = await groups.instantiate("widgets.manager.listitem", 1);
    assert.equal(groups.getnumitems(), 1);
    assert.equal(groups.enumitem(0), item);
    assert.equal(groups._div.children.length, 1);
    assert.equal(item.findobject("row.title").gettext(), "Nested title");
    groups.setredraw(0);
    assert.equal(groups._div.style.overflow, "hidden");
    groups.scrolltopercent(50);
    groups.removeall();
    assert.equal(groups.getnumitems(), 0);
  } finally {
    restoreBrowser();
  }
});
