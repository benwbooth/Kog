import GuiObj from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/GuiObj";
import Group from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Group";
import Button from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Button";
import ToggleButton from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/ToggleButton";
import SystemObject from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/SystemObject";
import { isMakiPromise, runMakiGenerator } from "./maki-execution.js";
import { interpret } from "../../../native/webamp/packages/webamp-modern/src/maki/interpreter";
import { classResolver } from "../../../native/webamp/packages/webamp-modern/src/skin/resolver";
import { renderedMakiString } from "./maki-locales.js";

const depths = new WeakMap<object, number>();

function effectiveVisibility(object: any): boolean {
  let current = object;
  const seen = new Set();
  while (current) {
    if (seen.has(current)) throw new Error("Cyclic MAKI object parent");
    seen.add(current);
    if (current._visible === false) return false;
    const parent = current._parent;
    if (current._isLayout && parent?._activeLayout && parent._activeLayout !== current) return false;
    current = parent;
  }
  return true;
}

function setVisibility(object: any, visible: boolean) {
  if (object._visible === visible) return;
  const descendants: any[] = [];
  const visit = (node: any) => {
    if (descendants.length >= 50000) throw new Error("MAKI visibility tree limit exceeded");
    descendants.push(node);
    for (const child of node._children ?? []) visit(child);
  };
  visit(object);
  const before = descendants.map(effectiveVisibility);
  object._visible = visible;
  object._renderVisibility();
  descendants.forEach((child, index) => {
    const after = effectiveVisibility(child);
    if (before[index] !== after) child.onsetvisible(after);
  });
}

// onAction is a callable script event with an integer result, not a count of
// listeners. Each callback gets its own stack; nested events can await safely.
function onaction(this: any, action: string, param: string, x: number, y: number, p1: number, p2: number, source: unknown) {
  const receiver = this;
  const root = this._uiRoot;
  return runMakiGenerator((function* () {
  const depth = depths.get(root) ?? 0;
  if (depth >= 64) throw new Error("MAKI action nesting limit exceeded");
  depths.set(root, depth + 1);
  let result = 0;
  try {
    for (const script of root.vm._scripts) {
      for (const binding of script.bindings) {
        if (script.methods[binding.methodOffset].name.toLowerCase() !== "onaction") continue;
        const variable = script.variables[binding.variableOffset];
        const matches = variable.isClass
          ? variable.members.some((index: number) => script.variables[index].value === receiver)
          : variable.type === "OBJECT" && variable.value === receiver;
        if (!matches) continue;
        if (variable.isClass) variable.value = receiver;
        const args: any[] = [
          { type: "STRING", value: action }, { type: "STRING", value: param },
          { type: "INT", value: x }, { type: "INT", value: y },
          { type: "INT", value: p1 }, { type: "INT", value: p2 },
          { type: "OBJECT", value: source },
        ];
        const execution = interpret(binding.commandOffset, script, args.reverse(), classResolver, "onaction", root);
        const returned = isMakiPromise(execution) ? yield execution : execution;
        if (returned && Number.isFinite(Number(returned.value))) result = Number(returned.value) | 0;
      }
    }
    return result;
  } finally {
    depths.set(root, depth);
  }
  })());
}

export function installMakiActionEvents() {
  const dispatchSystemEvent = (system: any, event: string, values: string[]) => {
    const result = system._uiRoot.vm.dispatch(system, event, values.map(value => ({ type: "STRING", value })));
    return isMakiPromise(result) ? result.then(() => undefined) : undefined;
  };
  // These native event entry points are also callable by MAKI scripts. A
  // self-call must reach that System instance's script bindings immediately.
  (SystemObject.prototype as any).onsetxuiparam = function (key: string, value: string) {
    return dispatchSystemEvent(this, "onsetxuiparam", [key, value]);
  };
  (SystemObject.prototype as any).ontitlechange = function (title: string) {
    return dispatchSystemEvent(this, "ontitlechange", [title]);
  };
  const activate = Button.prototype.setactivated;
  Button.prototype.setactivated = function (value: boolean) {
    // Wasabi ButtonWnd only emits onActivateButton when the value changes.
    if (Boolean(value) !== Boolean(this._active)) activate.call(this, value);
  };
  ToggleButton.prototype._cfgAttribChanged = function (value: string) {
    // Reloading configuration updates activation, not the user-toggle event.
    // onToggle is emitted by the input handler and must not feed config reloads
    // back into mutually exclusive settings.
    this.setactivated((Number.parseInt(value, 10) || 0) !== 0);
  };
  // Button's constructor installs its mousedown handler before GuiObj.init
  // installs the MAKI callback. Do not change activation before scripts can
  // inspect the pre-press state (ClassicPro uses that state to select tabs).
  ToggleButton.prototype._handleMouseDown = function (event: MouseEvent) {
    event.stopPropagation();
  };
  ToggleButton.prototype.onLeftButtonDown = function (x: number, y: number) {
    GuiObj.prototype.onLeftButtonDown.call(this, x, y);
    this.setactivated(!this._active);
    this.updateCfgAttib(this._active ? "1" : "0");
    this.ontoggle(this._active);
  };
  Group.prototype.getobject = function (id: string) {
    const key = String(id).toLowerCase();
    for (const child of this._children) {
      if (child.getId().toLowerCase() === key) return child;
    }
    return null;
  };
  GuiObj.prototype.show = function () { setVisibility(this, true); };
  GuiObj.prototype.hide = function () { setVisibility(this, false); };
  GuiObj.prototype.isvisible = function () { return effectiveVisibility(this); };
  const setAttribute = GuiObj.prototype.setXmlAttr;
  const updateTooltip = (object: any) => {
    object._div.setAttribute("title", renderedMakiString(object, object._tooltip ?? ""));
  };
  GuiObj.prototype.setXmlAttr = function (key: string, value: string) {
    const lower = key.toLowerCase();
    if (lower === "userdata" || lower === "translate") {
      (this as any)[`_${lower}`] = value ?? "";
      if (lower === "translate") {
        updateTooltip(this);
        (this as any)._renderText?.();
      }
      return true;
    }
    const handled = setAttribute.call(this, key, value);
    if (lower === "tooltip") updateTooltip(this);
    return handled;
  };
  const draw = GuiObj.prototype.draw;
  GuiObj.prototype.draw = function () {
    draw.call(this);
    updateTooltip(this);
  };
  const getAttribute = GuiObj.prototype.getxmlparam;
  GuiObj.prototype.getxmlparam = function (key: string) { return getAttribute.call(this, key) ?? ""; };
  const find = GuiObj.prototype.findobject;
  GuiObj.prototype.findobject = function (id: string) {
    const found = find.call(this, id);
    if (!found) console.warn(`MAKI lookup: ${this.getId()} -> ${id}`);
    return found;
  };
  (GuiObj.prototype as any).onaction = onaction;
  (GuiObj.prototype as any).sendaction = function (action: string, param: string, x: number, y: number, p1: number, p2: number) {
    return onaction.call(this, action, param, x, y, p1, p2, this);
  };
}
