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
    this.style = { setProperty(key, value) { this[key] = value; }, removeProperty(key) { delete this[key]; } };
    const classes = new Set();
    this.classList = { add(name) { classes.add(name); }, remove(name) { classes.delete(name); }, contains(name) { return classes.has(name); } };
  }
  addEventListener() {}
  appendChild(child) { this.children.push(child); return child; }
  append(...children) { this.children.push(...children); }
  remove() { this.removed = true; }
  setAttribute() {}
  getBoundingClientRect() { return { width: 100, height: 20, left: 0, top: 0 }; }
}

class FakeDocument {
  constructor() { this.body = new FakeElement(this, "body"); }
  createElement(name) { return new FakeElement(this, name); }
  addEventListener() {}
  removeEventListener() {}
}

class FakeAudioNode {
  constructor() { this.frequency = { value: 0 }; this.gain = { value: 0 }; this.pan = { value: 0 }; }
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
  const previous = {
    document: globalThis.document,
    localStorage: globalThis.localStorage,
    StereoPannerNode: globalThis.StereoPannerNode,
    window: globalThis.window,
  };
  const values = new Map();
  const localStorage = {
    getItem(key) { return values.get(String(key)) ?? null; },
    setItem(key, value) { values.set(String(key), String(value)); },
    removeItem(key) { values.delete(String(key)); },
  };
  globalThis.document = document;
  globalThis.localStorage = localStorage;
  globalThis.window = { AudioContext: FakeAudioContext, document, localStorage, requestAnimationFrame() {} };
  globalThis.StereoPannerNode = FakeAudioNode;
  return () => Object.assign(globalThis, previous);
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
      sourcefile: "maki-lifecycle-fixture.ts",
      contents: `
        import { installMakiActionEvents } from "./src/maki-events.ts";
        export { installMakiGeometry } from "./src/maki-geometry.ts";
        export { installMakiConfigBindings } from "./src/maki-config.ts";
        export { default as ConfigItem } from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/ConfigItem.ts";
        export { default as Slider } from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Slider.ts";
        export { default as Layout } from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Layout.ts";
        export { default as SystemObject } from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/SystemObject.ts";
        export { default as ToggleButton } from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/ToggleButton.ts";
        import { beginMakiStartup, finishMakiStartup, installMakiStartup, resumeMakiTimers } from "./src/maki-startup.ts";
        import GuiObj from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/GuiObj.ts";
        import Group from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Group.ts";
        import Timer from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Timer.ts";
        import Vm from "../../native/webamp/packages/webamp-modern/src/skin/VM.ts";
        export { beginMakiStartup, finishMakiStartup, GuiObj, Group, installMakiActionEvents, installMakiStartup, resumeMakiTimers, Timer, Vm };
      `,
    },
  });
  return import(`data:text/javascript;base64,${Buffer.from(result.outputFiles[0].contents).toString("base64")}`);
}

function makeRoot(Vm) {
  const root = { getContainers: () => [], vm: null };
  root.vm = new Vm(root);
  return root;
}

test("GUI config binding preserves script defaults and slider reload never writes back", async () => {
  const restoreBrowser = installBrowserGlobals(new FakeDocument());
  try {
    const { GuiObj, Vm, ConfigItem, Slider, installMakiConfigBindings } = await loadFixture();
    installMakiConfigBindings();
    const root = makeRoot(Vm);
    const values = new Map();
    let writes = 0;
    const config = {
      getValue: (_guid, key) => values.get(key),
      setValue: (_guid, key, value) => { writes++; values.set(key, value); },
    };
    const item = new ConfigItem(root, config, "Options", "test");
    root.CONFIG = { getitem: () => item };
    const object = new GuiObj(root);
    const updates = [];
    object._cfgAttribChanged = value => updates.push(value);
    object._setConfigAttrib("test;Enabled");
    assert.equal(writes, 0);
    const attribute = item.newattribute("Enabled", "1");
    assert.equal(attribute, object._configAttrib);
    assert.equal(attribute.getdata(), "1");
    object._setConfigAttrib("test;Other");
    attribute.setdata("0");
    assert.deepEqual(updates, [], "rebinding removes the old listener");
    const slider = Object.create(Slider.prototype);
    slider._high = 100;
    let renders = 0;
    slider._renderThumbPosition = () => renders++;
    const priorWrites = writes;
    slider._cfgAttribChanged(undefined);
    assert.equal(slider._position, 0);
    slider._cfgAttribChanged("75");
    assert.equal(slider._position, 0.75);
    assert.equal(renders, 2);
    assert.equal(writes, priorWrites);
  } finally {
    restoreBrowser();
  }
});

test("MAKI geometry resolves nested relative dimensions and independent bounds", async () => {
  const restoreBrowser = installBrowserGlobals(new FakeDocument());
  try {
    const { GuiObj, Group, Vm, SystemObject, Layout, installMakiGeometry } = await loadFixture();
    installMakiGeometry();
    document.documentElement = { clientWidth: 1000, clientHeight: 679 };
    assert.equal(SystemObject.prototype.getcurappwidth(), 1000);
    assert.equal(SystemObject.prototype.getcurappheight(), 679);
    assert.equal(SystemObject.prototype.getviewportwidthfromguiobject(null), 1000);
    assert.equal(SystemObject.prototype.getviewportheightfrompoint(20, 30), 679);
    const root = makeRoot(Vm);
    const layout = Object.create(Layout.prototype);
    layout._parent = { _x: 354, _y: 165 };
    layout._x = 0; layout._y = 0;
    assert.equal(layout.getguix(), 354);
    assert.equal(layout.getguiy(), 165);
    const parent = new Group(root);
    parent._w = 500; parent._h = 400;
    const child = new GuiObj(root);
    child._parent = parent;
    child._w = -30; child._h = -50;
    child._relatw = "1"; child._relath = "1";
    assert.equal(child.getwidth(), 470);
    assert.equal(child.getheight(), 350);
    parent._w = 0;
    parent._background = "natural";
    root.getBitmap = () => ({ getWidth: () => 500, getHeight: () => 400 });
    assert.equal(child.getwidth(), 470, "relative sizes retain parent bitmap dimensions");
    parent._w = 500;
    child._maximumWidth = 450;
    assert.equal(child.getwidth(), 450);
    assert.equal(child.getheight(), 350);
    child._x = -20; child._y = -10;
    child._relatx = "1"; child._relaty = "1";
    assert.equal(child.getleft(), 480);
    assert.equal(child.gettop(), 390);
    child.setXmlAttr("fitparent", "1");
    assert.equal(child._h, 0);
    assert.equal(child.getheight(), 400);
    child.setXmlAttr("fitparent", "-5");
    assert.equal(child._x, 5);
    assert.equal(child._w, -10);
    assert.equal(child.getheight(), 390);
    child.setXmlAttr("fitparent", "0");
    assert.equal(child._w, -10);
    const nested = new GuiObj(root);
    nested._parent = child; nested._w = -10; nested._relatw = "1";
    assert.equal(nested.getwidth(), 440);
    parent._parent = nested; parent._relatw = "1";
    assert.throws(() => nested.getwidth(), /Cyclic MAKI geometry parent/);
  } finally { restoreBrowser(); }
});

function listen(vm, object, event, callback) {
  const script = {
    bindings: [{ methodOffset: 0, variableOffset: 0, commandOffset: 0 }],
    methods: [{ name: event }],
    variables: [{ type: "OBJECT", value: object, isClass: false }],
    callback,
  };
  vm._scripts.push(script);
}

test("MAKI config reload does not synthesize user toggles or repeated activation", async () => {
  const restoreBrowser = installBrowserGlobals(new FakeDocument());
  try {
    const { ToggleButton, Vm, installMakiActionEvents } = await loadFixture();
    installMakiActionEvents();
    const root = makeRoot(Vm);
    const events = [];
    root.vm.dispatch = (_object, name, args) => events.push([name, args?.[0]?.value]);
    const button = new ToggleButton(root);
    button.setactivated(true);
    button.setactivated(true);
    assert.deepEqual(events.map(([name]) => name), ["onactivate"]);
    button._cfgAttribChanged("1");
    assert.equal(events.length, 1);
    button._cfgAttribChanged("0");
    assert.deepEqual(events.map(([name]) => name), ["onactivate", "onactivate"]);
    assert.equal(button.getactivated(), false);
    button.ontoggle(true);
    assert.equal(events.at(-1)[0], "ontoggle");
  } finally { restoreBrowser(); }
});

test("initially visible GUI objects receive one post-init visibility event", async () => {
  const restoreBrowser = installBrowserGlobals(new FakeDocument());
  try {
    const { beginMakiStartup, finishMakiStartup, GuiObj, Group, installMakiActionEvents,
      installMakiStartup, resumeMakiTimers, Timer, Vm } = await loadFixture();
    installMakiActionEvents();
    installMakiStartup();
    const root = makeRoot(Vm);
    const layout = new Group(root);
    const visible = new Group(root);
    const hidden = new GuiObj(root);
    const revealed = new GuiObj(root);
    hidden._visible = false;
    revealed._visible = false;
    layout.addChild(visible);
    layout.addChild(hidden);
    visible.addChild(revealed);
    const container = { getVisible: () => true, getcurlayout: () => layout };
    root.getContainers = () => [container];
    const timer = new Timer(root);
    timer._delay = 60_000;
    const events = [];
    const revealedEvents = [];
    listen(root.vm, visible, "onsetvisible", (_name, args) => {
      events.push(args[0].value);
      revealed.show();
      timer.start();
    });
    listen(root.vm, revealed, "onsetvisible", (_name, args) => revealedEvents.push(args[0].value));
    listen(root.vm, hidden, "onsetvisible", () => assert.fail("hidden windows must not get initial visibility"));
    root.vm.interpret = async (script, _offset, event, args) => script.callback(event, args);
    beginMakiStartup(root);
    await finishMakiStartup(root);
    await finishMakiStartup(root);
    assert.deepEqual(events, [1]);
    assert.deepEqual(revealedEvents, [1], "a nested show already supplies the child's initial visibility");
    assert.equal(timer.isrunning(), true, "ClassicPro-style visibility callback starts its deferred timer");
    assert.equal(timer._timeout, null);
    await resumeMakiTimers(root);
    assert.notEqual(timer._timeout, null);
    timer.stop();
  } finally { restoreBrowser(); }
});

test("MAKI lifecycle preserves effective visibility and defers startup events/timers", async () => {
  const restoreBrowser = installBrowserGlobals(new FakeDocument());
  try {
    const {
      beginMakiStartup, finishMakiStartup, GuiObj, Group, installMakiActionEvents,
      installMakiStartup, resumeMakiTimers, Timer, Vm,
    } = await loadFixture();
    installMakiActionEvents();
    installMakiStartup();

    const root = makeRoot(Vm);
    const parent = new Group(root);
    const child = new GuiObj(root);
    const grandchild = new GuiObj(root);
    parent.addChild(child);
    child._children.push(grandchild);
    grandchild.setParent(child);
    const visibility = [];
    for (const object of [parent, child, grandchild]) {
      object.onsetvisible = visible => visibility.push([object, visible]);
    }

    child.hide();
    assert.equal(child._visible, false);
    assert.equal(child.isvisible(), false);
    parent.hide();
    // The already-hidden child and its descendant do not receive a duplicate
    // event merely because an ancestor changed effective visibility.
    assert.deepEqual(visibility.map(([, visible]) => visible), [false, false, false]);
    assert.equal(visibility.at(-1)[0], parent, "only the changed ancestor receives its second hide notification");
    child.show();
    assert.equal(child._visible, true, "the local visible flag is independent of ancestors");
    assert.equal(child.isvisible(), false, "a visible child remains effectively hidden");
    parent.show();
    assert.equal(child.isvisible(), true);
    assert.equal(grandchild.isvisible(), true);
    assert.deepEqual(visibility.map(([, visible]) => visible), [false, false, false, true, true, true]);
    parent.hide();
    assert.deepEqual(visibility.slice(-3).map(([, visible]) => visible), [false, false, false]);
    parent._visible = true;
    parent._parent = grandchild;
    assert.throws(() => parent.isvisible(), /Cyclic MAKI object parent/);
    parent._parent = undefined;

    const target = new GuiObj(root);
    target.setXmlAttr("id", "startup.target");
    const received = [];
    let releaseScript;
    const scriptGate = new Promise(resolve => { releaseScript = resolve; });
    for (const event of ["onresize", "onsetvisible", "onsetxuiparam"]) {
      listen(root.vm, target, event, (_name, args) => received.push([event, args.map(value => ({ ...value }))]));
    }
    listen(root.vm, target, "onscriptloaded", async () => {
      received.push(["onscriptloaded", []]);
      await scriptGate;
    });
    root.vm.interpret = async (script, _offset, event, args) => script.callback(event, args);

    beginMakiStartup(root);
    const firstResize = [{ type: "INT", value: 1 }];
    const finalResize = [{ type: "INT", value: 2 }];
    const xuiArgs = [{ type: "STRING", value: "before-mutation" }];
    root.vm.dispatch(target, "onresize", firstResize);
    root.vm.dispatch(target, "onresize", finalResize);
    root.vm.dispatch(target, "onsetvisible", [{ type: "BOOLEAN", value: 1 }]);
    root.vm.dispatch(target, "onsetxuiparam", xuiArgs);
    finalResize[0].value = 99;
    xuiArgs[0].value = "mutated";
    const scriptLoaded = root.vm.dispatch(target, "onscriptloaded");
    const finishing = finishMakiStartup(root);
    await Promise.resolve();
    assert.deepEqual(received, [
      ["onsetvisible", [{ type: "BOOLEAN", value: 1 }]],
      ["onsetxuiparam", [{ type: "STRING", value: "before-mutation" }]],
      ["onscriptloaded", []],
    ], "visibility and XUI notifications run at the time of the change; resize waits for startup");
    releaseScript();
    await Promise.all([scriptLoaded, finishing]);
    assert.deepEqual(received, [
      ["onsetvisible", [{ type: "BOOLEAN", value: 1 }]],
      ["onsetxuiparam", [{ type: "STRING", value: "before-mutation" }]],
      ["onscriptloaded", []],
      ["onresize", [{ type: "INT", value: 2 }]],
    ]);

    const cancelled = new Timer(root);
    cancelled._delay = 60_000;
    assert.equal(cancelled.start(), true);
    assert.equal(cancelled._timeout, null, "timers do not arm before resume");
    assert.equal(cancelled.isrunning(), true);
    cancelled.stop();
    const resumed = new Timer(root);
    resumed._delay = 60_000;
    resumed.start();
    await resumeMakiTimers(root);
    assert.notEqual(resumed._timeout, null, "resume starts only timers that remain queued");
    assert.equal(cancelled._timeout, null, "stop removes a timer from the startup queue");
    resumed.stop();
  } finally {
    restoreBrowser();
  }
});
