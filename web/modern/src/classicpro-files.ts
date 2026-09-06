import parseXml from "@rgrove/parse-xml";
import File from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/File";
import MakiList from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/List";
import XmlDoc from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/XmlDoc";

// `File` is synchronous in the MAKI ABI.  The caller supplies the only resource
// authority this shim has: the already-loaded skin/ClassicPro resource maps.
// It deliberately never falls back to fetch, XMLHttpRequest, or a host path.
export type ClassicProResourceGetter = (root: unknown, path: string) => Uint8Array | null;

type FileState = {
  bytes: Uint8Array | null;
  path: string;
};

type XmlState = FileState & {
  callbacks: string[];
};

type MakiFile = InstanceType<typeof File> & {
  _uiRoot: unknown;
  _path: string;
};

type MakiXmlDoc = InstanceType<typeof XmlDoc> & MakiFile;

type XmlElement = {
  attributes: Record<string, string>;
  children: XmlNode[];
  name: string;
  type: string;
};

type XmlNode = XmlElement | { type: string };

const MAX_XML_BYTES = 1024 * 1024;
const MAX_XML_DEPTH = 64;
const MAX_XML_NODES = 8192;
const MAX_XML_ATTRIBUTES = 128;

const fileStates = new WeakMap<object, FileState>();
const xmlStates = new WeakMap<object, XmlState>();
let resourceGetter: ClassicProResourceGetter | null = null;
let installed = false;

function fileState(file: MakiFile): FileState {
  const prior = fileStates.get(file);
  if (prior) return prior;
  const state = { bytes: null, path: "" };
  fileStates.set(file, state);
  return state;
}

function xmlState(doc: MakiXmlDoc): XmlState {
  const prior = xmlStates.get(doc);
  if (prior) return prior;
  const state = { ...fileState(doc), callbacks: [] };
  xmlStates.set(doc, state);
  return state;
}

function load(this: MakiFile, path: string): void {
  const value = String(path ?? "");
  const getter = resourceGetter;
  if (!getter) throw new Error("ClassicPro File compatibility has not been installed.");
  const bytes = value ? getter(this._uiRoot, value) : null;
  if (bytes != null && !(bytes instanceof Uint8Array)) {
    throw new Error(`ClassicPro resource getter returned invalid bytes for ${value}.`);
  }
  this._path = value;
  const state = fileState(this);
  state.path = value;
  state.bytes = bytes;
  const xml = xmlStates.get(this);
  if (xml) {
    xml.path = value;
    xml.bytes = bytes;
  }
}

function exists(this: MakiFile): boolean {
  return fileState(this).bytes != null;
}

function getsize(this: MakiFile): number {
  return fileState(this).bytes?.byteLength ?? 0;
}

function isXmlElement(node: XmlNode): node is XmlElement {
  return node.type === "element";
}

function callbackMatches(pattern: string, path: string): boolean {
  // Winamp documents `*` as the wildcard and XML paths as uppercase.  Preserve
  // literal Wasabi `:` tag names rather than interpreting them as namespaces.
  const expression = pattern
    .toUpperCase()
    .split("*")
    .map(part => part.replace(/[|\\{}()[\]^$+?.]/g, "\\$&"))
    .join(".*");
  return new RegExp(`^${expression}$`).test(path);
}

function dispatch(doc: MakiXmlDoc, event: string, args: unknown[]): void {
  const root = thisRoot(doc);
  // VM dispatch is intentionally used instead of calling JavaScript methods:
  // parser callbacks are MAKI event bindings on this exact XmlDoc instance.
  void root.vm.dispatch(doc, event, args);
}

function thisRoot(doc: MakiXmlDoc): { vm: { dispatch: (object: unknown, event: string, args: unknown[]) => unknown } } {
  const root = doc._uiRoot as { vm?: { dispatch?: unknown } } | null;
  if (!root || !root.vm || typeof root.vm.dispatch !== "function") {
    throw new Error("ClassicPro XmlDoc has no MAKI VM dispatcher.");
  }
  return root as { vm: { dispatch: (object: unknown, event: string, args: unknown[]) => unknown } };
}

function dispatchOpen(doc: MakiXmlDoc, path: string, element: XmlElement): void {
  const names = new MakiList(doc._uiRoot as never);
  const values = new MakiList(doc._uiRoot as never);
  for (const [name, value] of Object.entries(element.attributes)) {
    names.additem(name);
    values.additem(value);
  }
  dispatch(doc, "parser_oncallback", [
    { type: "STRING", value: path },
    { type: "STRING", value: element.name },
    { type: "OBJECT", value: names },
    { type: "OBJECT", value: values },
  ]);
}

function dispatchClose(doc: MakiXmlDoc, path: string, element: XmlElement): void {
  dispatch(doc, "parser_onclosecallback", [
    { type: "STRING", value: path },
    { type: "STRING", value: element.name },
  ]);
}

function dispatchError(doc: MakiXmlDoc, error: unknown): void {
  const state = xmlState(doc);
  const detail = error instanceof Error ? error : new Error(String(error));
  const line = typeof (detail as Error & { line?: unknown }).line === "number"
    ? (detail as Error & { line: number }).line
    : 0;
  dispatch(doc, "parser_onerror", [
    { type: "STRING", value: state.path },
    { type: "INT", value: line },
    { type: "STRING", value: "" },
    { type: "INT", value: 1 },
    { type: "STRING", value: detail.message },
  ]);
}

function visit(
  doc: MakiXmlDoc,
  node: XmlNode,
  parentPath: string,
  depth: number,
  budget: { nodes: number },
): void {
  if (!isXmlElement(node)) return;
  if (depth > MAX_XML_DEPTH) throw new Error(`ClassicPro XML exceeds nesting limit (${MAX_XML_DEPTH}).`);
  if (++budget.nodes > MAX_XML_NODES) throw new Error(`ClassicPro XML exceeds element limit (${MAX_XML_NODES}).`);
  if (Object.keys(node.attributes).length > MAX_XML_ATTRIBUTES) {
    throw new Error(`ClassicPro XML element ${node.name} exceeds attribute limit (${MAX_XML_ATTRIBUTES}).`);
  }

  const path = `${parentPath}/${node.name}`.replace(/^\//, "").toUpperCase();
  const callbacks = xmlState(doc).callbacks;
  const matched = callbacks.some(pattern => callbackMatches(pattern, path));
  if (matched) dispatchOpen(doc, path, node);
  for (const child of node.children) visit(doc, child, path, depth + 1, budget);
  // The SDK specifies that self-closing tags receive the close event too.
  if (matched) dispatchClose(doc, path, node);
}

function parser_addcallback(this: MakiXmlDoc, section: string): void {
  const pattern = String(section ?? "").trim();
  if (!pattern) throw new Error("ClassicPro XmlDoc callback path must not be empty.");
  xmlState(this).callbacks.push(pattern);
}

function parser_start(this: MakiXmlDoc): void {
  const state = xmlState(this);
  try {
    if (!state.bytes) throw new Error(`ClassicPro XML resource is not loaded: ${state.path || "(empty path)"}.`);
    if (state.bytes.byteLength > MAX_XML_BYTES) {
      throw new Error(`ClassicPro XML exceeds ${MAX_XML_BYTES} byte limit: ${state.path}.`);
    }
    const text = new TextDecoder("utf-8", { fatal: true }).decode(state.bytes);
    const document = parseXml(text) as { children: XmlNode[] };
    for (const child of document.children) visit(this, child, "", 1, { nodes: 0 });
  } catch (error) {
    dispatchError(this, error);
    // An invalid local skin resource is a real failure, not an empty document.
    throw error;
  }
}

function parser_destroy(this: MakiXmlDoc): string {
  xmlState(this).callbacks = [];
  return "";
}

/**
 * Installs the MAKI File/XmlDoc subset used by ClassicPro. `getResource` must
 * resolve only preloaded, sandbox-approved bytes for the supplied UI root.
 */
export function installClassicProFileApi(getResource: ClassicProResourceGetter): void {
  resourceGetter = getResource;
  if (installed) return;
  installed = true;

  File.prototype.load = load;
  File.prototype.exists = exists;
  File.prototype.getsize = getsize;
  XmlDoc.prototype.parser_addcallback = parser_addcallback;
  XmlDoc.prototype.parser_start = parser_start;
  XmlDoc.prototype.parser_destroy = parser_destroy;
}
