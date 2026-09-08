import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import { readFile, readdir } from "node:fs/promises";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import path from "node:path";
import esbuild from "esbuild";
import { adaptMakiResolver, adaptMakiSource } from "../maki-compat.mjs";

const directory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryDirectory = path.resolve(directory, "../..");
const interpreterPath = path.join(
  repositoryDirectory,
  "native/webamp/packages/webamp-modern/src/maki/interpreter.ts",
);
const membersPath = path.join(directory, "src/maki-members.js");

class FakeElement {
  constructor(name) {
    this.localName = name;
    this.children = [];
    this.attributes = new Map();
    this.listeners = new Map();
    this.style = {
      setProperty(key, value) { this[key] = value; },
      removeProperty(key) { delete this[key]; },
      getPropertyValue(key) { return this[key] ?? ""; },
    };
    const classes = new Set();
    this.classList = {
      add: (...names) => names.forEach(name => classes.add(name)),
      contains: name => classes.has(name),
      remove: (...names) => names.forEach(name => classes.delete(name)),
      toggle: (name, force) => {
        const enabled = force ?? !classes.has(name);
        if (enabled) classes.add(name);
        else classes.delete(name);
        return enabled;
      },
    };
  }

  addEventListener(name, callback) {
    const callbacks = this.listeners.get(name) ?? [];
    callbacks.push(callback);
    this.listeners.set(name, callbacks);
  }

  removeEventListener(name, callback) {
    this.listeners.set(name, (this.listeners.get(name) ?? []).filter(item => item !== callback));
  }

  appendChild(child) {
    if (child.parentElement) {
      child.parentElement.children = child.parentElement.children.filter(item => item !== child);
    }
    this.children.push(child);
    child.parentElement = this;
    return child;
  }

  setAttribute(name, value) { this.attributes.set(name, String(value)); }
  getBoundingClientRect() { return { left: 0, top: 0, width: 100, height: 20 }; }
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
  createAnalyser() { return new FakeAudioNode(); }
  createBiquadFilter() { return new FakeAudioNode(); }
}

const document = {
  body: new FakeElement("body"),
  createElement(name) { return new FakeElement(name); },
  addEventListener() {},
  removeEventListener() {},
};
const localStorage = new FakeStorage();
globalThis.document = document;
globalThis.localStorage = localStorage;
globalThis.StereoPannerNode = FakeAudioNode;
globalThis.window = { document, localStorage, AudioContext: FakeAudioContext, requestAnimationFrame() {} };

function replaceInstructionLoop(source) {
  const original = "    let ip = start;\n    while (ip < this.commands.length) {\n      const command = this.commands[ip];";
  const bounded = `    let ip = start;
    let remainingInstructions = 100_000;
    while (ip < this.commands.length) {
      if (--remainingInstructions < 0) {
        throw new Error(\`MAKI instruction budget exceeded in \${this.maki_id}\`);
      }
      const command = this.commands[ip];`;
  assert.ok(source.includes(original), "Pinned Webamp MAKI interpreter changed");
  return source.replace(original, bounded);
}

async function loadRuntime() {
  const result = await esbuild.build({
    bundle: true,
    format: "esm",
    nodePaths: [path.join(directory, "node_modules")],
    platform: "node",
    write: false,
    stdin: {
      loader: "ts",
      resolveDir: directory,
      sourcefile: "maki-compat-fixture.ts",
      contents: `
        export { parse } from "../../native/webamp/packages/webamp-modern/src/maki/parser.ts";
        export { interpret } from "../../native/webamp/packages/webamp-modern/src/maki/interpreter.ts";
        export { default as GuiObj } from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/GuiObj.ts";
        export { default as Group } from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Group.ts";
        export { default as ToggleButton } from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/ToggleButton.ts";
        export { installClassicProBrowser } from "./src/classicpro-browser.ts";
        export { makiInterface } from "./src/maki-members.js";
        export { installMakiActionEvents } from "./src/maki-events.ts";
        export { default as Vm } from "../../native/webamp/packages/webamp-modern/src/skin/VM.ts";
        export { installMakiDispatch } from "./src/maki-dispatch.ts";
        export { classResolver } from "../../native/webamp/packages/webamp-modern/src/skin/resolver.ts";
        export { default as SystemObject } from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/SystemObject.ts";
        export { installClassicProServices } from "./src/classicpro-services.ts";
      `,
    },
    plugins: [{
      name: "test-production-maki-adaptations",
      setup(context) {
        context.onLoad({ filter: /[/\\]skin[/\\]resolver\.ts$/ }, async args => ({
          contents: adaptMakiResolver(await readFile(args.path, "utf8"), path.join(directory, "src/classicpro-services.ts")),
          loader: "ts",
        }));
        context.onLoad({ filter: /[/\\]maki[/\\](constants|parser)\.ts$/ }, async args => ({
          contents: adaptMakiSource(path.basename(args.path, ".ts"), await readFile(args.path, "utf8")),
          loader: "ts",
        }));
        context.onLoad({ filter: /[/\\]maki[/\\]interpreter\.ts$/ }, async args => {
          let contents = await readFile(args.path, "utf8");
          if (path.resolve(args.path) === interpreterPath) {
            contents = adaptMakiSource("interpreter", replaceInstructionLoop(contents), membersPath);
          }
          return { contents, loader: "ts" };
        });
      },
    }],
  });
  return import(`data:text/javascript;base64,${Buffer.from(result.outputFiles[0].contents).toString("base64")}`);
}

const originalLog = console.log;
console.log = () => {};
let runtime;
try {
  runtime = await loadRuntime();
} finally {
  console.log = originalLog;
}

function encodeString(value) {
  const bytes = Buffer.from(value, "utf8");
  const length = Buffer.alloc(2);
  length.writeUInt16LE(bytes.length);
  return Buffer.concat([length, bytes]);
}

function encodeGuid(guid) {
  assert.match(guid, /^[0-9a-f]{32}$/i);
  const result = Buffer.alloc(16);
  for (let index = 0; index < 4; index += 1) {
    result.writeUInt32LE(Number.parseInt(guid.slice(index * 8, index * 8 + 8), 16), index * 4);
  }
  return result;
}

function u32(value) {
  const result = Buffer.alloc(4);
  result.writeUInt32LE(value);
  return result;
}

function command(opcode, argument) {
  return argument == null
    ? Buffer.from([opcode])
    : Buffer.concat([Buffer.from([opcode]), u32(argument)]);
}

function makeMaki({ classes = [], methods = [], variables = [], constants = [], bindings = [], commands = [] }) {
  const parts = [Buffer.from("FG"), Buffer.from([3, 4]), u32(23), u32(classes.length)];
  parts.push(...classes.map(encodeGuid), u32(methods.length));
  for (const method of methods) {
    const descriptor = Buffer.alloc(4);
    descriptor.writeUInt16LE(method.classIndex);
    parts.push(descriptor, encodeString(method.name));
  }
  parts.push(u32(variables.length));
  for (const variable of variables) {
    const descriptor = Buffer.alloc(14);
    descriptor.writeUInt8(variable.type ?? variable.classIndex, 0);
    descriptor.writeUInt8(variable.classIndex == null ? 0 : 1, 1);
    descriptor.writeUInt16LE(variable.value ?? 0, 4);
    descriptor.writeUInt8(variable.global ? 1 : 0, 12);
    descriptor.writeUInt8(variable.isStatic ? 1 : 0, 13);
    parts.push(descriptor);
  }
  parts.push(u32(constants.length));
  for (const constant of constants) parts.push(u32(constant.variableIndex), encodeString(constant.value));
  parts.push(u32(bindings.length));
  for (const binding of bindings) {
    parts.push(u32(binding.variableIndex), u32(binding.methodIndex), u32(binding.byteOffset));
  }
  const code = Buffer.concat(commands);
  parts.push(u32(code.length), code);
  return Buffer.concat(parts);
}

function asArrayBuffer(buffer) {
  return buffer.buffer.slice(buffer.byteOffset, buffer.byteOffset + buffer.byteLength);
}

const MEMBER_GUID = "4ee3e1994becc636bc78cd97b028869c";

function userMemberProgram() {
  return makeMaki({
    classes: [MEMBER_GUID],
    variables: [
      { classIndex: 0, isStatic: true },
      { type: 6 },
      { type: 2, value: 41 },
      { type: 6 },
    ],
    constants: [
      { variableIndex: 1, value: "Counter" },
      { variableIndex: 3, value: "counter" },
    ],
    commands: [
      command(1, 0), command(1, 1), command(104, 2), command(1, 2), command(48), command(33),
      command(1, 0), command(1, 3), command(104, 2), command(33),
    ],
  });
}

test("real parser decodes and interpreter persists OPCODE_UMV reference variables", async () => {
  const bytes = userMemberProgram();
  const program = runtime.parse(asArrayBuffer(bytes), "handcrafted-umv.maki");
  assert.equal(program.commands[2].opcode, 104);
  assert.equal(program.commands[2].arg, 2);
  assert.equal(program.variables[0].isStatic, true);

  const target = {};
  program.variables[0].value = target;
  const resolver = guid => guid === MEMBER_GUID ? class {} : null;
  assert.equal(await runtime.interpret(0, program, [], resolver, "setter", {}), program.variables[2]);
  const member = await runtime.interpret(6, program, [], resolver, "getter", {});
  assert.deepEqual(member, { type: "INT", value: 41 });
  assert.equal(await runtime.interpret(6, program, [], resolver, "getter", {}), member);

  const otherScript = runtime.parse(asArrayBuffer(bytes), "isolated-umv.maki");
  otherScript.variables[0].value = target;
  assert.deepEqual(await runtime.interpret(6, otherScript, [], resolver, "getter", {}), { type: "INT", value: 0 });
});

test("top-level MAKI return yields the actual INT variable", async () => {
  const bytes = makeMaki({ variables: [{ type: 2, value: 73 }], commands: [command(1, 0), command(33)] });
  const program = runtime.parse(asArrayBuffer(bytes), "return-int.maki");
  assert.equal(runtime.interpret(0, program, [], () => null, "test", {}), program.variables[0],
    "synchronous bytecode must not add a Promise turn between native callbacks");
  assert.deepEqual(program.variables[0], { global: 0, type: "INT", value: 73 });
});

test("MAKI logical operators normalize their complete numeric truth tables", async () => {
  for (const lhs of [0, 1, 2]) for (const rhs of [0, 1, 2]) for (const opcode of [80, 81]) {
    const bytes = makeMaki({ variables: [{ type: 2, value: lhs }, { type: 2, value: rhs }],
      commands: [command(1, 0), command(1, 1), command(opcode), command(33)] });
    const program = runtime.parse(asArrayBuffer(bytes), "logical-truth-table.maki");
    const result = await runtime.interpret(0, program, [], () => null, "logic", {});
    assert.deepEqual(result, { type: "BOOLEAN", value: Number(opcode === 80 ? !!lhs && !!rhs : !!lhs || !!rhs) },
      `${lhs} ${opcode === 80 ? '&&' : '||'} ${rhs}`);
  }
});

test("nested dispatch finishes every synchronous listener before a caller clears its guard", () => {
  runtime.installMakiDispatch();
  const vm = Object.create(runtime.Vm.prototype);
  const target = {};
  const calls = [];
  let guarded = false;
  vm._scripts = [{
    variables: [{ type: "OBJECT", value: target }],
    methods: [{ name: "outer" }, { name: "inner" }],
    bindings: [
      { variableOffset: 0, methodOffset: 0, commandOffset: 0 },
      { variableOffset: 0, methodOffset: 1, commandOffset: 1 },
      { variableOffset: 0, methodOffset: 1, commandOffset: 2 },
    ],
  }];
  vm.interpret = (_script, offset) => {
    if (offset === 0) {
      guarded = true;
      assert.equal(vm.dispatch(target, "inner"), 2);
      guarded = false;
      calls.push("outer finished");
    } else {
      assert.equal(guarded, true, "all nested callbacks must observe the active guard");
      calls.push(`inner ${offset}`);
    }
    return { type: "INT", value: 0 };
  };
  assert.equal(vm.dispatch(target, "outer"), 1);
  assert.deepEqual(calls, ["inner 1", "inner 2", "outer finished"]);
});

test("interpreter calls do not consume a shared event-argument stack", async () => {
  const bytes = makeMaki({
    variables: [{ type: 2 }, { type: 2, value: 17 }],
    commands: [command(3, 0), command(1, 1), command(33)],
  });
  const program = runtime.parse(asArrayBuffer(bytes), "shared-event-stack.maki");
  const sharedArgs = [{ type: "INT", value: 9 }];
  assert.deepEqual(await runtime.interpret(0, program, sharedArgs, () => null, "first", {}), { global: 0, type: "INT", value: 17 });
  assert.deepEqual(sharedArgs, [{ type: "INT", value: 9 }]);
  assert.deepEqual(await runtime.interpret(0, program, sharedArgs, () => null, "second", {}), { global: 0, type: "INT", value: 17 });
  assert.equal(program.variables[0].value, 9);
});

test("an OBJECT method returning null stays null rather than wrapping a MAKI variable", async () => {
  class NullFinder { findobject(_id) { return null; } }
  const bytes = makeMaki({
    classes: [MEMBER_GUID],
    methods: [{ classIndex: 0, name: "findObject" }],
    variables: [{ classIndex: 0 }, { classIndex: 0 }, { type: 6 }],
    constants: [{ variableIndex: 2, value: "missing" }],
    commands: [command(1, 0), command(1, 2), command(24, 0), command(33)],
  });
  const program = runtime.parse(asArrayBuffer(bytes), "null-object.maki");
  program.variables[0].value = new NullFinder();
  const result = await runtime.interpret(0, program, [], () => NullFinder, "test", {});
  assert.deepEqual(result, { type: "OBJECT", value: null });
});

test("native Promise-returning methods suspend and resume the real interpreter", async () => {
  for (const asynchronous of [false, true]) {
    let release;
    const pending = new Promise(resolve => { release = resolve; });
    class Finder {}
    Finder.prototype.findobject = asynchronous
      ? async function (_id) { return await pending; }
      : function (_id) { return pending; };
    const bytes = makeMaki({
      classes: [MEMBER_GUID],
      methods: [{ classIndex: 0, name: "findObject" }],
      variables: [{ classIndex: 0 }, { type: 6 }],
      constants: [{ variableIndex: 1, value: "pending" }],
      commands: [command(1, 0), command(1, 1), command(24, 0), command(33)],
    });
    const program = runtime.parse(asArrayBuffer(bytes), "async-native.maki");
    program.variables[0].value = new Finder();
    const result = runtime.interpret(0, program, [], () => Finder, "test", {});
    assert.equal(typeof result.then, "function");
    release(null);
    assert.deepEqual(await result, { type: "OBJECT", value: null });
  }
});

test("native calls resolve a group's declared embedded control interface", async () => {
  runtime.installClassicProBrowser();
  const root = { getImageManager() { return {}; }, vm: { dispatch() {} } };
  const group = new runtime.Group(root);
  const button = new runtime.ToggleButton(root);
  button.setXmlAttr("id", "cpro.tab.button");
  group.addChild(button);
  group.setXmlAttr("embed_xui", "cpro.tab.button");
  assert.equal(runtime.makiInterface(group, runtime.GuiObj), group, "preserve the wrapper's own GUI interface");
  assert.equal(runtime.makiInterface(group, runtime.ToggleButton), button);
  assert.equal(group.setactivated, undefined, "the wrapper does not acquire fabricated button methods");
  const unrelated = new runtime.Group(root);
  assert.equal(runtime.makiInterface(unrelated, runtime.ToggleButton), unrelated,
    "a group without declared embedded content cannot masquerade as a button");
  const bytes = makeMaki({
    classes: [runtime.Group.GUID, runtime.ToggleButton.GUID],
    methods: [{ classIndex: 1, name: "setActivated" }],
    variables: [{ classIndex: 0 }, { type: 2, value: 1 }],
    commands: [command(1, 0), command(1, 1), command(24, 0), command(33)],
  });
  const program = runtime.parse(asArrayBuffer(bytes), "embedded-toggle.maki");
  program.variables[0].value = group;
  await runtime.interpret(0, program, [], runtime.classResolver, "test", root);
  assert.equal(button.getactivated(), true, "MAKI activates the actual embedded toggle button");
  assert.equal(program.variables[0].value, group, "interface dispatch preserves the original group reference");
});

test("typed MAKI assignment selects the embedded control while Group assignment preserves its wrapper", async () => {
  runtime.installClassicProBrowser();
  const root = { getImageManager() { return {}; }, vm: { dispatch() {} } };
  const group = new runtime.Group(root);
  const button = new runtime.ToggleButton(root);
  button.setXmlAttr("id", "button");
  group.addChild(button);
  group.setXmlAttr("embed_xui", "button");
  for (const opcode of [3, 48]) {
    const assignments = opcode === 3
      ? [command(1, 0), command(3, 1), command(1, 0), command(3, 2)]
      : [command(1, 1), command(1, 0), command(48), command(2), command(1, 2), command(1, 0), command(48), command(2)];
    const bytes = makeMaki({
      classes: [runtime.Group.GUID, runtime.ToggleButton.GUID],
      methods: [{ classIndex: 1, name: "setActivated" }],
      variables: [{ classIndex: 0 }, { classIndex: 1 }, { classIndex: 0 }, { type: 2, value: 1 }],
      commands: [...assignments, command(1, 1), command(1, 3), command(24, 0), command(33)],
    });
    const program = runtime.parse(asArrayBuffer(bytes), `embedded-assignment-${opcode}.maki`);
    program.variables[0].value = group;
    await runtime.interpret(0, program, [], runtime.classResolver, "test", root);
    assert.equal(program.variables[1].value, button, "typed variable binds events to the embedded control");
    assert.equal(program.variables[2].value, group, "Group variable retains wrapper identity");
    assert.equal(button.getactivated(), true);
    button.setactivated(false);
  }
});

test("GuiObj.init(parent) uses its one-argument ABI and mounts a real Group", async () => {
  const groupGuid = runtime.Group.GUID;
  const bytes = makeMaki({
    classes: [MEMBER_GUID, groupGuid],
    methods: [{ classIndex: 0, name: "init" }],
    variables: [{ classIndex: 1 }, { classIndex: 0 }, { classIndex: 1 }],
    commands: [command(1, 0), command(1, 2), command(24, 0), command(33)],
  });
  const program = runtime.parse(asArrayBuffer(bytes), "gui-init.maki");
  const root = { getBitmap() { return null; }, vm: { dispatch() {} } };
  const child = new runtime.Group(root);
  const parent = new runtime.Group(root);
  program.variables[0].value = child;
  program.variables[2].value = parent;

  const warnings = [];
  const originalWarn = console.warn;
  console.warn = (...args) => warnings.push(args);
  try {
    const result = await runtime.interpret(
      0,
      program,
      [],
      guid => guid === MEMBER_GUID ? runtime.GuiObj : runtime.Group,
      "test",
      root,
    );
    assert.deepEqual(result, { type: "NULL", value: undefined });
  } finally {
    console.warn = originalWarn;
  }

  assert.equal(child._parent, parent);
  assert.deepEqual(parent._children, [child]);
  assert.deepEqual(parent.getDiv().children, [child.getDiv()]);
  assert.equal(child.getDiv().classList.contains("webamp--img"), true);
  assert.equal(child._inited, true);
  assert.equal(child.getDiv().listeners.has("mousedown"), true);
  assert.deepEqual(warnings, [], "GuiObj.init must not swallow an afterInited TypeError");
});

test("onAction listeners get independent stacks and return the last finite INT", async () => {
  runtime.installMakiActionEvents();
  const root = { vm: { _scripts: [] } };
  const object = new runtime.GuiObj(root);

  function actionProgram(result) {
    const bytes = makeMaki({
      classes: [MEMBER_GUID],
      methods: [{ classIndex: 0, name: "onAction" }],
      variables: [{ classIndex: 0 }, { classIndex: 0 }, { type: 2, value: result }, { type: 6 }],
      bindings: [{ variableIndex: 0, methodIndex: 0, byteOffset: 0 }],
      commands: [command(3, 3), command(1, 2), command(33)],
    });
    const program = runtime.parse(asArrayBuffer(bytes), `onaction-${result}.maki`);
    program.variables[0].value = object;
    return program;
  }

  const first = actionProgram(11);
  const second = actionProgram(29);
  root.vm._scripts.push(first, second);
  assert.equal(await object.onaction("play", "now", 1, 2, 3, 4, object), 29);
  assert.equal(first.variables[3].value, "play");
  assert.equal(second.variables[3].value, "play");
});

test("MAKI can synchronously call its own System XUI and title event bindings", () => {
  runtime.installMakiActionEvents();
  runtime.installMakiDispatch();
  for (const { method, args } of [
    { method: "onSetXuiParam", args: ["bgcolor", "10,20,30"] },
    { method: "onTitleChange", args: ["Updated title"] },
  ]) {
    const firstConstant = 1 + args.length;
    const commands = [
      command(1, 0),
      ...args.map((_, index) => command(1, firstConstant + index)).reverse(),
      command(24, 0), command(33),
    ];
    const callbackOffset = Buffer.concat(commands).length;
    commands.push(...args.map((_, index) => command(3, index + 1)), command(33));
    const bytes = makeMaki({
      classes: [runtime.SystemObject.GUID],
      methods: [{ classIndex: 0, name: method }],
      variables: [{ classIndex: 0 }, ...args.map(() => ({ type: 6 })), ...args.map(() => ({ type: 6 }))],
      constants: args.map((value, index) => ({ variableIndex: firstConstant + index, value })),
      bindings: [{ variableIndex: 0, methodIndex: 0, byteOffset: callbackOffset }],
      commands,
    });
    const program = runtime.parse(asArrayBuffer(bytes), `self-${method}.maki`);
    const root = { vm: Object.create(runtime.Vm.prototype) };
    root.vm._uiRoot = root;
    root.vm._scripts = [program];
    const system = Object.create(runtime.SystemObject.prototype);
    system._uiRoot = root;
    program.variables[0].value = system;
    const result = runtime.interpret(0, program, [], runtime.classResolver, "test", root);
    assert.deepEqual(result, { type: "NULL", value: undefined });
    assert.deepEqual(program.variables.slice(1, firstConstant).map(variable => variable.value), args);
  }
});

async function makiFilesBelow(root) {
  const files = [];
  for (const entry of await readdir(root, { withFileTypes: true })) {
    const fullPath = path.join(root, entry.name);
    if (entry.isDirectory()) files.push(...await makiFilesBelow(fullPath));
    else if (entry.name.toLowerCase().endsWith(".maki")) files.push(fullPath);
  }
  return files;
}

test("production resolver recognizes the real NowPlaying widget's Color calls", async () => {
  runtime.installClassicProServices();
  const root = path.join(directory, "vendor/classicpro/engine");
  let colorCalls = 0;
  for (const filename of [path.join(root, "widgets/Data/NowPlaying/NowPlaying.maki")]) {
    const bytes = await readFile(filename);
    const program = runtime.parse(asArrayBuffer(bytes), path.relative(root, filename));
    const calledClasses = new Set(program.commands.filter(command => command.opcode === 24 || command.opcode === 112)
      .map(command => program.classes[program.methods[command.arg].typeOffset]));
    for (const guid of calledClasses) {
      let resolved;
      try { resolved = runtime.classResolver(guid); }
      catch (error) { assert.fail(`${filename}: ${guid}: ${error.message}`); }
      assert.equal(typeof resolved, "function", `${filename}: ${guid}`);
      if (guid === "95ddb2214e2b00e33583a58e103cc148") colorCalls++;
    }
  }
  assert.ok(colorCalls > 0, "real shipped Color references must exercise the resolver");
});

test("at least one shipped ClassicPro program exercises real OPCODE_UMV bytes", async () => {
  const root = path.join(directory, "vendor/classicpro/engine");
  const matches = [];
  const originalWarn = console.warn;
  console.warn = () => {};
  try {
    for (const filename of await makiFilesBelow(root)) {
      const bytes = await readFile(filename);
      try {
        const program = runtime.parse(asArrayBuffer(bytes), path.relative(root, filename));
        if (program.commands.some(item => item.opcode === 104)) matches.push(path.relative(root, filename));
      } catch {
        // Some ClassicPro scripts declare private extension classes absent from
        // Webamp's standard object table. They are unrelated to opcode decoding.
      }
    }
  } finally {
    console.warn = originalWarn;
  }
  assert.ok(matches.length > 0, "the vendored ClassicPro bytecode should cover OPCODE_UMV");
});
