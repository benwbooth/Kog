import SkinEngineWAL from "../../../native/webamp/packages/webamp-modern/src/skin/SkinEngine_WAL";
import SystemObject from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/SystemObject";
import Group from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Group";
import GuiObj from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/GuiObj";
import { XmlElement } from "@rgrove/parse-xml";
import { CPRO_ROOT } from "./classicpro.js";

const definitions = new WeakMap<object, Map<string, { engine: any; node: any }>>();

export function installMakiDynamicContainers() {
  const traverse = SkinEngineWAL.prototype.traverseChild;
  SkinEngineWAL.prototype.traverseChild = async function (node, parent) {
    if (node.name?.toLowerCase() === "guiobject") return this.newGui(GuiObj, node, parent);
    if (node.name?.toLowerCase() === "winamp:browser") {
      if (!this._uiRoot.getXuiElement("winamp:browser")) {
        await this.include(new XmlElement("include", { file: `${CPRO_ROOT}__wasabi__/xml/xui/browser/browser.xml` }), null);
      }
      return this.dynamicXuiElement(node, parent);
    }
    return traverse.call(this, node, parent);
  };
  const container = SkinEngineWAL.prototype.container;
  SkinEngineWAL.prototype.container = async function (node) {
    let entries = definitions.get(this._uiRoot);
    if (!entries) definitions.set(this._uiRoot, entries = new Map());
    entries.set(String(node.attributes.id).toLowerCase(), { engine: this, node });
    // Wasabi records dynamic templates during static loading; only an
    // explicitly default-visible template is instantiated at startup.
    if (Number.parseInt(node.attributes.dynamic, 10)
        && !Number.parseInt(node.attributes.default_visible, 10)) return null;
    return container.call(this, node);
  };
  (SystemObject.prototype as any).newdynamiccontainer = async function (id: string) {
    const root = this._uiRoot;
    const definition = definitions.get(root)?.get(id.toLowerCase());
    if (!definition) return null;
    if (root.getContainers().length >= 128) throw new Error("MAKI container limit exceeded");
    const instance = await container.call(definition.engine, definition.node);
    instance.draw();
    root.getRootDiv().appendChild(instance.getDiv());
    instance.init();
    return instance;
  };
  (SystemObject.prototype as any).newgroup = async function (id: string) {
    const definition = definitions.get(this._uiRoot)?.values().next().value;
    if (!definition || !this._uiRoot.getGroupDef(id)) return null;
    const group = new Group(this._uiRoot);
    await definition.engine.maybeApplyGroupDef(group, new XmlElement("group", { id }));
    return group;
  };
}
