import parseXml from "@rgrove/parse-xml";

// ClassicPro references Winamp paths; these are virtual resource identifiers,
// never filesystem paths or network URLs.
export const CPRO_ROOT = "__kog_classicpro__/";

export function classicProPath(reference) {
  const path = String(reference || "").replaceAll("\\", "/").toLowerCase();
  const match = path.match(/^(?:@colorthemespath@\/\.\.\/\.\.\/|@winamppath@\/|)plugins\/classicpro\/engine\/(.+)$/);
  const relative = path.startsWith(CPRO_ROOT) ? path.slice(CPRO_ROOT.length) : match?.[1];
  if (!relative || relative.split("/").some(part => !part || part === "." || part === "..") || /[:?#%]/.test(relative)) return null;
  return CPRO_ROOT + relative;
}

export function engineIncludes(reference, parent, files) {
  const direct = classicProPath(reference);
  const normalized = String(reference).replaceAll("\\", "/").toLowerCase();
  let resolved = direct;
  if (!resolved && parent?.startsWith(CPRO_ROOT) && !/^[@/]|[:?#%]/.test(normalized)) {
    const parts = parent.slice(CPRO_ROOT.length).split("/").filter(Boolean);
    for (const part of normalized.split("/")) {
      if (!part || part === ".") continue;
      if (part === "..") {
        if (!parts.length) throw new Error("ClassicPro include escapes the engine resources");
        parts.pop();
      } else parts.push(part);
    }
    resolved = classicProPath(CPRO_ROOT + parts.join("/"));
  }
  if (!resolved) return null;
  const key = resolved.slice(CPRO_ROOT.length);
  if (!key.includes("*")) return [resolved];
  // Only the engine's directory-local XML wildcard is supported.
  if (!key.endsWith("/*.xml") || key.slice(0, -5).includes("*")) throw new Error("Unsupported ClassicPro include wildcard");
  const directory = key.slice(0, -5);
  return Object.keys(files).filter(file => file.startsWith(directory) && !file.slice(directory.length).includes("/") && file.endsWith(".xml")).sort().map(file => CPRO_ROOT + file);
}

export function prepareClassicProXml(text, filePath, files) {
  const enginePath = classicProPath(filePath);
  if (!enginePath && !/classicpro/i.test(text)) return text;
  const parent = enginePath ? enginePath.slice(0, enginePath.lastIndexOf("/") + 1) : null;
  // Wasabi XUI tags use colons without namespace declarations. Browser
  // DOMParser rejects these valid Wasabi documents; use the renderer's parser.
  const xml = parseXml(`<wrapper>${text.replace(/<\?xml[^?]*\?>/gi, "")}</wrapper>`);
  const escape = value => value.replaceAll("&", "&amp;").replaceAll('"', "&quot;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
  const serialize = node => {
    if (node.type === "text" || node.type === "cdata") return escape(node.text);
    if (node.type !== "element") return "";
    if (node.name.toLowerCase() === "include") {
      const includes = engineIncludes(node.attributes.file, parent, files);
      if (includes) return includes.map(file => `<include file="${escape(file)}"/>`).join("");
    }
    const attributes = Object.entries(node.attributes).map(([key, value]) => ` ${key}="${escape(String(value))}"`).join("");
    return `<${node.name}${attributes}>${node.children.map(serialize).join("")}</${node.name}>`;
  };
  return xml.children[0].children.map(serialize).join("");
}
