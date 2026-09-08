import { getClass, normalizedObjects } from "../../../native/webamp/packages/webamp-modern/src/maki/objects";
import type { UIRoot } from "../../../native/webamp/packages/webamp-modern/src/UIRoot";
import type SkinColor from "../../../native/webamp/packages/webamp-modern/src/skin/Color";
import BaseObject from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/BaseObject";
import type SystemObject from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/SystemObject";

type MakiClass = new (...args: any[]) => BaseObject;
type Rgb = readonly [number, number, number];
type LifecycleState = {
  managers: Set<ClassicProColorManager>;
  phase: number;
  singleton?: ClassicProColorManager;
};

type ObjectDefinition = (typeof normalizedObjects)[string];
type MethodDefinition = ObjectDefinition["functions"][number];

// The interpreter reads GUID words in their little-endian MAKI representation.
export const CLASSICPRO_COLOR_MANAGER_GUID = "aee235ff498febd1e0d7af961a54d4da";
export const CLASSICPRO_COLOR_GUID = "95ddb2214e2b00e33583a58e103cc148";

const COLOR_MANAGER_OBJECT_ID = "aee235ffebd1498f96afd7e0dad4541a";
const COLOR_OBJECT_ID = "95ddb22100e34e2b8ea5833548c13c10";
const SYSTEM_GUID = "d6f50f6449b793fa66baf193983eaeef";
const LIFECYCLE_EVENTS = [
  "onbeforeloadingelements",
  "onguiloaded",
  "onloaded",
] as const;

const lifecycleByRoot = new WeakMap<UIRoot, LifecycleState>();

function lifecycleState(uiRoot: UIRoot): LifecycleState {
  let state = lifecycleByRoot.get(uiRoot);
  if (!state) {
    state = { managers: new Set(), phase: -1 };
    lifecycleByRoot.set(uiRoot, state);
  }
  return state;
}

export function resolveRgb(
  uiRoot: UIRoot,
  color: SkinColor,
  seen = new Set<string>(),
): { rgb: Rgb; gammaGroup: string } {
  const id = color.getId().toLowerCase();
  if (seen.has(id)) {
    throw new Error(`ClassicPro color alias cycle at ${color.getId()}`);
  }
  seen.add(id);

  const value = color.getValue().trim();
  const components = value.split(",").map((component) => Number(component.trim()));
  if (
    components.length === 3 &&
    components.every((component) => Number.isInteger(component))
  ) {
    if (components.some((component) => component < 0 || component > 255)) {
      throw new Error(`ClassicPro color ${color.getId()} is outside the RGB range`);
    }
    return {
      rgb: components as unknown as Rgb,
      gammaGroup: color.getGammaGroup() || "",
    };
  }

  const target = uiRoot.getColor(value);
  if (!target) {
    throw new Error(`ClassicPro color ${color.getId()} references missing color ${value}`);
  }
  const resolved = resolveRgb(uiRoot, target, seen);
  return {
    rgb: resolved.rgb,
    gammaGroup: color.getGammaGroup() || resolved.gammaGroup,
  };
}

export class ClassicProColor extends BaseObject {
  static GUID = CLASSICPRO_COLOR_GUID;

  private readonly uiRoot: UIRoot;
  private readonly color: SkinColor;

  constructor(uiRoot: UIRoot, color: SkinColor) {
    super();
    this.uiRoot = uiRoot;
    this.color = color;
    this._id = color.getId();
  }

  private rgb(withGamma: boolean): Rgb {
    const resolved = resolveRgb(this.uiRoot, this.color);
    if (!withGamma) return resolved.rgb;
    const gamma = this.uiRoot._getGammaGroup(resolved.gammaGroup);
    if (!gamma) {
      throw new Error(`ClassicPro gamma group ${resolved.gammaGroup} is unavailable`);
    }
    const [red, green, blue] = gamma.transformColor2rgb(resolved.rgb.join(","));
    return [Math.round(red), Math.round(green), Math.round(blue)];
  }

  getred(): number {
    return this.rgb(false)[0];
  }

  getgreen(): number {
    return this.rgb(false)[1];
  }

  getblue(): number {
    return this.rgb(false)[2];
  }

  getredwithgamma(): number {
    return this.rgb(true)[0];
  }

  getgreenwithgamma(): number {
    return this.rgb(true)[1];
  }

  getbluewithgamma(): number {
    return this.rgb(true)[2];
  }
}

export class ClassicProColorManager extends BaseObject {
  static GUID = CLASSICPRO_COLOR_MANAGER_GUID;

  readonly _uiRoot: UIRoot;
  _deliveredLifecyclePhase = -1;

  constructor(uiRoot: UIRoot) {
    super();
    this._uiRoot = uiRoot;
    this._id = "ColorMgr";
    lifecycleState(uiRoot).managers.add(this);
  }

  getcolor(id: string): ClassicProColor {
    const color = this._uiRoot.getColor(id);
    if (!color) throw new Error(`ClassicPro color ${id} is unavailable`);
    return new ClassicProColor(this._uiRoot, color);
  }

  dispose() {
    lifecycleState(this._uiRoot).managers.delete(this);
  }
}

export const classicProColorClasses: Readonly<Record<string, MakiClass>> =
  Object.freeze({
    [CLASSICPRO_COLOR_MANAGER_GUID]: ClassicProColorManager,
    [CLASSICPRO_COLOR_GUID]: ClassicProColor,
  });

function method(
  name: string,
  result: string,
  parameters: string[][] = [],
): MethodDefinition {
  return { name, result, parameters, deprecated: false };
}

function registerObject(id: string, definition: ObjectDefinition) {
  const existing = normalizedObjects[id];
  if (existing && existing.name !== definition.name) {
    throw new Error(`MAKI object ${id} is already registered as ${existing.name}`);
  }
  normalizedObjects[id] = definition;
}

export function registerClassicProColorMetadata() {
  registerObject(COLOR_MANAGER_OBJECT_ID, {
    parent: "System",
    name: "ColorMgr",
    parentClass: getClass(SYSTEM_GUID),
    deprecated: false,
    functions: [
      method("getColor", "Color", [["String", "colorID"]]),
      method("onBeforeLoadingElements", ""),
      method("onGuiLoaded", ""),
      method("onLoaded", ""),
    ],
  });
  registerObject(COLOR_OBJECT_ID, {
    parent: "Object",
    name: "Color",
    parentClass: getClass(BaseObject.GUID),
    deprecated: false,
    functions: [
      method("getRed", "int"),
      method("getGreen", "int"),
      method("getBlue", "int"),
      method("getRedWithGamma", "int"),
      method("getGreenWithGamma", "int"),
      method("getBlueWithGamma", "int"),
    ],
  });
}

function singletonFor(uiRoot: UIRoot): ClassicProColorManager {
  const state = lifecycleState(uiRoot);
  if (!state.singleton) state.singleton = new ClassicProColorManager(uiRoot);
  return state.singleton;
}

// Webamp currently discards the MAKI `system` bit used by `_predecl` objects.
// Call this before SystemObject.init() so ColorMgr.getColor() has its real
// per-UIRoot receiver when onScriptLoaded runs.
export function bindClassicProColorGlobals(systemObject: SystemObject): number {
  const manager = singletonFor(systemObject._uiRoot);
  let bound = 0;
  for (const variable of systemObject._parsedScript.variables) {
    if (
      variable.type === "OBJECT" &&
      variable.guid === CLASSICPRO_COLOR_MANAGER_GUID &&
      variable.isStatic === true &&
      variable.value == null
    ) {
      variable.value = manager;
      bound += 1;
    }
  }
  return bound;
}

async function notifyLifecycle(uiRoot: UIRoot, targetPhase: number) {
  const state = lifecycleState(uiRoot);
  state.phase = Math.max(state.phase, targetPhase);
  for (const manager of [...state.managers]) {
    while (manager._deliveredLifecyclePhase < state.phase) {
      manager._deliveredLifecyclePhase += 1;
      await uiRoot.vm.dispatch(
        manager,
        LIFECYCLE_EVENTS[manager._deliveredLifecyclePhase],
      );
    }
  }
}

export function notifyClassicProBeforeLoadingElements(uiRoot: UIRoot) {
  return notifyLifecycle(uiRoot, 0);
}

export function notifyClassicProGuiLoaded(uiRoot: UIRoot) {
  return notifyLifecycle(uiRoot, 1);
}

export function notifyClassicProLoaded(uiRoot: UIRoot) {
  return notifyLifecycle(uiRoot, 2);
}

registerClassicProColorMetadata();
