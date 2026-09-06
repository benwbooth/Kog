import Edit from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Edit";
import Group from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Group";
import GroupList from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/GroupList";
import GuiList from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/GuiList";
import SkinEngineWAL from "../../../native/webamp/packages/webamp-modern/src/skin/SkinEngine_WAL";
import { XmlElement } from "@rgrove/parse-xml";

type MakiValue = { type: "INT"; value: number };

type ListItem = {
  icon: string;
  labels: string[];
};

type ListColumn = {
  dynamic: boolean;
  label: string;
  width: number;
};

type ListState = {
  autoDeselect: boolean;
  columns: ListColumn[];
  columnLabels: string | null;
  columnWidths: string | null;
  fontSize: number;
  focus: number;
  iconHeight: number;
  iconWidth: number;
  items: ListItem[];
  lastAdded: number;
  multiSelect: boolean;
  preventMultiple: boolean;
  selected: Set<number>;
  showColumns: boolean;
  showIcons: boolean;
};

type EditState = {
  autoEnter: boolean;
  idleEnabled: boolean;
  idleTimer: ReturnType<typeof setTimeout> | null;
  input: HTMLInputElement | null;
  text: string;
};

type GroupListState = {
  groups: Group[];
  maxHeight: number;
  redraw: boolean;
  scrollPercent: number;
  scrollY: number;
};

type CustomObjectState = {
  content: Group | null;
  declaredGroupId: string | null;
  generation: number;
  groupId: string;
  pending: Promise<Group | null> | null;
};

type GroupMaterializer = {
  _uiRoot: object;
  newGroup: (Type: typeof Group, node: XmlElement, parent: unknown) => Promise<Group>;
};

type GuiObjectLike = {
  _div: HTMLElement;
  _uiRoot?: {
    getBitmap?: (id: string) => { setAsBackground?: (element: HTMLElement) => void } | null;
    vm?: { dispatch?: (object: unknown, event: string, args: MakiValue[]) => unknown };
  };
  _children?: unknown[];
};

const listStates = new WeakMap<object, ListState>();
const editStates = new WeakMap<object, EditState>();
const groupListStates = new WeakMap<object, GroupListState>();
const customObjectStates = new WeakMap<object, CustomObjectState>();
const materializers = new WeakMap<object, GroupMaterializer>();
let installed = false;

function integer(value: unknown, fallback = 0): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? Math.trunc(parsed) : fallback;
}

function bool(value: unknown): boolean {
  return integer(value) !== 0 || value === true || String(value).toLowerCase() === "true";
}

function listState(list: object): ListState {
  const prior = listStates.get(list);
  if (prior) return prior;
  const state: ListState = {
    autoDeselect: false,
    columns: [{ dynamic: true, label: "", width: -1 }],
    columnLabels: null,
    columnWidths: null,
    fontSize: 12,
    focus: -1,
    iconHeight: 16,
    iconWidth: 16,
    items: [],
    lastAdded: -1,
    multiSelect: false,
    preventMultiple: false,
    selected: new Set(),
    showColumns: true,
    showIcons: false,
  };
  listStates.set(list, state);
  return state;
}

function editState(edit: object): EditState {
  const prior = editStates.get(edit);
  if (prior) return prior;
  const state = { autoEnter: false, idleEnabled: true, idleTimer: null, input: null, text: "" };
  editStates.set(edit, state);
  return state;
}

function groupListState(list: object): GroupListState {
  const prior = groupListStates.get(list);
  if (prior) return prior;
  const state = { groups: [], maxHeight: 0, redraw: true, scrollPercent: 0, scrollY: 0 };
  groupListStates.set(list, state);
  return state;
}

function customObjectState(holder: object): CustomObjectState {
  const prior = customObjectStates.get(holder);
  if (prior) return prior;
  const state = { content: null, declaredGroupId: null, generation: 0, groupId: "", pending: null };
  customObjectStates.set(holder, state);
  return state;
}

/**
 * Wasabi's <CustomObject> is a GuiObjectWnd whose GROUPID parameter replaces
 * the window's content with a GroupMgr-instantiated group.  The Webamp engine
 * materializer is asynchronous, so hold the requested XML value synchronously
 * and do the replacement only through that same materializer.
 */
class CustomObject extends Group {
  setXmlAttr(key: string, value: string): boolean {
    if (key.toLowerCase() === "groupid") {
      customObjectState(this).declaredGroupId = String(value ?? "");
      return true;
    }
    return super.setXmlAttr(key, value);
  }

  async setxmlparam(key: string, value: string): Promise<void> {
    if (key.toLowerCase() !== "groupid") {
      super.setxmlparam(key, value);
      return;
    }
    const materializer = materializers.get(this._uiRoot);
    if (!materializer) {
      throw new Error("ClassicPro CustomObject.setXmlParam(groupid) requires installClassicProControls() before the skin engine builds its UI.");
    }
    await materializeCustomObject(this, String(value ?? ""), materializer);
  }
}

function removeCustomObjectContent(holder: CustomObject, state: CustomObjectState): void {
  const content = state.content;
  if (!content) return;
  content.dispose();
  content.getDiv().remove();
  const childIndex = holder._children.indexOf(content);
  if (childIndex >= 0) holder._children.splice(childIndex, 1);
  state.content = null;
}

async function materializeCustomObject(
  holder: CustomObject,
  groupId: string,
  materializer: GroupMaterializer,
): Promise<Group | null> {
  const state = customObjectState(holder);
  if (state.groupId === groupId) return state.pending ?? state.content;

  const root = holder._uiRoot as { getGroupDef?: (id: string) => XmlElement | null };
  // SkinEngineWAL intentionally permits a plain <group> with an unknown id;
  // CustomObject's contract is group-manager content, so do not claim that an
  // empty shell is the requested widget.
  if (groupId && (typeof root.getGroupDef !== "function" || !root.getGroupDef(groupId))) {
    throw new Error(`ClassicPro CustomObject cannot materialize unknown groupid: ${groupId}`);
  }
  const token = ++state.generation;
  state.groupId = groupId;
  removeCustomObjectContent(holder, state);
  if (!groupId) return null;

  const pending = materializer.newGroup(
    Group,
    new XmlElement("group", { id: groupId }),
    holder,
  ).then(group => {
    if (token !== state.generation) {
      group.dispose();
      group.getDiv().remove();
      const childIndex = holder._children.indexOf(group);
      if (childIndex >= 0) holder._children.splice(childIndex, 1);
      return null;
    }
    state.content = group;
    group.init();
    // Static XML content is drawn by its parent later. For an already-live
    // holder, draw and append the new group immediately, matching setContent.
    if (holder._inited) {
      group.draw();
      holder.getDiv().append(group.getDiv());
    }
    return group;
  }).finally(() => {
    if (state.generation === token) state.pending = null;
  });
  state.pending = pending;
  return pending;
}

function dispatch(object: GuiObjectLike, event: string, ...values: number[]): void {
  const vm = object._uiRoot?.vm;
  const send = vm?.dispatch;
  if (typeof send === "function") {
    void send.call(vm, object, event, values.map(value => ({ type: "INT", value })));
  }
}

function styleControlHost(host: HTMLElement): void {
  // Wasabi controls are child windows. Their XML x/y/w/h are therefore a
  // positioned border box. Absolute positioning blockifies custom elements;
  // do not write display here because GuiObj owns display:none visibility.
  host.style.position = "absolute";
  host.style.boxSizing = "border-box";
}

function validItem(state: ListState, position: unknown): number {
  const index = integer(position, -1);
  return index >= 0 && index < state.items.length ? index : -1;
}

function setColumnCount(state: ListState, value: unknown): void {
  const count = Math.max(1, Math.min(256, integer(value, 1)));
  state.columns = Array.from({ length: count }, (_, index) =>
    state.columns[index] ?? { dynamic: true, label: "", width: -1 });
  if (state.columnWidths !== null) setColumnWidths(state, state.columnWidths);
  if (state.columnLabels !== null) setColumnLabels(state, state.columnLabels);
}

function setColumnWidths(state: ListState, value: string): void {
  state.columnWidths = String(value ?? "");
  const widths = String(value ?? "").split(/[;,]/);
  for (let index = 0; index < Math.min(widths.length, state.columns.length); index++) {
    const width = integer(widths[index], -1);
    state.columns[index].dynamic = width < 0;
    state.columns[index].width = width;
  }
}

function setColumnLabels(state: ListState, value: string): void {
  state.columnLabels = String(value ?? "");
  const labels = String(value ?? "").split(";");
  for (let index = 0; index < Math.min(labels.length, state.columns.length); index++) {
    state.columns[index].label = labels[index];
  }
}

function columnTemplate(state: ListState): string {
  return state.columns.map(column => column.dynamic ? "minmax(0, 1fr)" : `${Math.max(0, column.width)}px`).join(" ");
}

function renderList(list: GuiObjectLike): void {
  const state = listState(list);
  const host = list._div;
  if (!host || !host.ownerDocument) return;

  styleControlHost(host);
  host.style.overflow = "auto";
  host.style.fontSize = `${state.fontSize}px`;
  host.setAttribute("role", "listbox");
  host.setAttribute("aria-multiselectable", String(state.multiSelect && !state.preventMultiple));

  const showHeader = state.showColumns && state.columns.some(column => column.label !== "");
  const header = showHeader ? host.ownerDocument.createElement("div") : null;
  if (header) {
    header.setAttribute("data-classicpro-columns", "1");
    header.setAttribute("role", "row");
    header.style.display = "grid";
    header.style.gridTemplateColumns = `${state.showIcons ? `${state.iconWidth}px ` : ""}${columnTemplate(state)}`;
    header.style.position = "sticky";
    header.style.top = "0";
    header.style.zIndex = "1";
    header.style.whiteSpace = "nowrap";
    if (state.showIcons) {
      const spacer = host.ownerDocument.createElement("span");
      spacer.setAttribute("aria-hidden", "true");
      header.append(spacer);
    }
    for (const column of state.columns) {
      const cell = host.ownerDocument.createElement("span");
      cell.setAttribute("role", "columnheader");
      cell.textContent = column.label;
      header.append(cell);
    }
  }

  const rows = state.items.map((item, index) => {
    const row = host.ownerDocument.createElement("div");
    row.setAttribute("role", "option");
    row.setAttribute("data-classicpro-row", String(index));
    row.setAttribute("aria-selected", String(state.selected.has(index)));
    row.style.minHeight = `${state.fontSize + 4}px`;
    row.style.display = "grid";
    row.style.gridTemplateColumns = `${state.showIcons ? `${state.iconWidth}px ` : ""}${columnTemplate(state)}`;
    row.style.whiteSpace = "pre";
    row.style.cursor = "default";
    if (state.selected.has(index)) row.setAttribute("data-selected", "1");
    if (index === state.focus) row.setAttribute("data-focused", "1");

    if (state.showIcons) {
      const icon = host.ownerDocument.createElement("span");
      icon.setAttribute("data-bitmap-id", item.icon);
      icon.classList.add("webamp--img");
      icon.style.display = "inline-block";
      icon.style.width = `${state.iconWidth}px`;
      icon.style.height = `${state.iconHeight}px`;
      icon.style.verticalAlign = "middle";
      icon.style.backgroundImage = "var(--background-image)";
      icon.style.backgroundRepeat = "no-repeat";
      // Bitmap owns the skin resource's transformed CSS variable. This does
      // not turn the skin-provided bitmap id into a URL or a DOM string.
      list._uiRoot?.getBitmap?.(item.icon)?.setAsBackground?.(icon);
      row.append(icon);
    }
    for (let column = 0; column < state.columns.length; column++) {
      const label = host.ownerDocument.createElement("span");
      // Labels originate in skin scripts and playlist metadata. textContent
      // keeps them data, rather than allowing markup into the UI.
      label.textContent = item.labels[column] ?? "";
      row.append(label);
    }

    row.addEventListener("click", event => {
      const mouse = event as MouseEvent;
      const preserve = state.multiSelect && !state.preventMultiple && (mouse.ctrlKey || mouse.metaKey);
      selectItem(list, index, preserve ? !state.selected.has(index) : true, !preserve);
      dispatch(list, "onleftclick", index);
    });
    row.addEventListener("dblclick", () => dispatch(list, "ondoubleclick", index));
    row.addEventListener("contextmenu", event => {
      event.preventDefault();
      selectItem(list, index, true, true);
      dispatch(list, "onrightclick", index);
    });
    return row;
  });
  host.replaceChildren(...(header ? [header, ...rows] : rows));
}

function selectItem(list: GuiObjectLike, position: number, selected: boolean, clearOther: boolean): void {
  const state = listState(list);
  if (validItem(state, position) < 0) return;
  if (clearOther || state.preventMultiple || !state.multiSelect) {
    for (const index of [...state.selected]) {
      if (index !== position) {
        state.selected.delete(index);
        dispatch(list, "onitemselection", index, 0);
      }
    }
  }
  const changed = state.selected.has(position) !== selected;
  if (selected) state.selected.add(position);
  else state.selected.delete(position);
  state.focus = position;
  if (changed) dispatch(list, "onitemselection", position, selected ? 1 : 0);
  renderList(list);
}

function syncEdit(edit: GuiObjectLike): HTMLInputElement | null {
  const state = editState(edit);
  if (state.input) return state.input;
  const host = edit._div;
  if (!host || !host.ownerDocument) return null;
  styleControlHost(host);
  host.style.overflow = "hidden";
  const input = host.ownerDocument.createElement("input");
  input.type = "text";
  input.value = state.text;
  input.setAttribute("data-classicpro-edit", "1");
  input.style.boxSizing = "border-box";
  input.style.width = "100%";
  input.style.height = "100%";
  input.style.minWidth = "0";
  input.style.margin = "0";
  input.style.padding = "0";
  input.style.border = "0";
  input.style.background = "transparent";
  input.style.color = "inherit";
  input.style.font = "inherit";
  input.addEventListener("input", () => {
    state.text = input.value;
    dispatch(edit, "oneditupdate");
    if (state.idleTimer !== null) clearTimeout(state.idleTimer);
    // EditWnd restarts its 350 ms idle timer after each text change.
    state.idleTimer = setTimeout(() => {
      state.idleTimer = null;
      if (state.idleEnabled) dispatch(edit, "onidleeditupdate");
    }, 350);
  });
  input.addEventListener("keydown", event => {
    if (event.key === "Enter") {
      event.preventDefault();
      dispatch(edit, "onenter");
    } else if (event.key === "Escape") {
      event.preventDefault();
      dispatch(edit, "onabort");
    }
  });
  input.addEventListener("focus", () => {
    if ((edit as { _autoselect?: boolean })._autoselect) input.select();
  });
  input.addEventListener("blur", () => {
    if (state.autoEnter) dispatch(edit, "onenter");
  });
  // Edits do not expose an HTML parser surface. Replacing only this control's
  // contents makes the native GuiObj host the positioning and clipping owner.
  host.replaceChildren(input);
  state.input = input;
  return input;
}

function repositionSelection(state: ListState, removed: number): void {
  const shifted = new Set<number>();
  for (const index of state.selected) {
    if (index < removed) shifted.add(index);
    else if (index > removed) shifted.add(index - 1);
  }
  state.selected = shifted;
  if (state.focus === removed) state.focus = -1;
  else if (state.focus > removed) state.focus--;
}

function listSetXmlAttr(this: GuiObjectLike, original: (key: string, value: string) => boolean, key: string, value: string): boolean {
  if (original.call(this, key, value)) return true;
  const state = listState(this);
  switch (key.toLowerCase()) {
    case "multiselect":
      state.multiSelect = bool(value);
      renderList(this);
      return true;
    case "fontsize":
      setFontSize.call(this, integer(value, state.fontSize));
      return true;
    case "numcolumns":
      setColumnCount(state, value);
      renderList(this);
      return true;
    case "columnwidths":
      setColumnWidths(state, value);
      renderList(this);
      return true;
    case "columnlabels":
      setColumnLabels(state, value);
      renderList(this);
      return true;
    case "nocolheader":
      state.showColumns = !bool(value);
      renderList(this);
      return true;
    case "showcolumns":
      state.showColumns = bool(value);
      renderList(this);
      return true;
    default:
      return false;
  }
}

function editSetXmlAttr(this: GuiObjectLike, original: (key: string, value: string) => boolean, key: string, value: string): boolean {
  if (original.call(this, key, value)) return true;
  switch (key.toLowerCase()) {
    case "text":
      setEditText.call(this, value);
      return true;
    case "autoselect":
      (this as { _autoselect?: boolean })._autoselect = bool(value);
      return true;
    default:
      return false;
  }
}

function addItem(this: GuiObjectLike, label: string): number {
  const state = listState(this);
  state.items.push({ icon: "", labels: [String(label ?? "")] });
  state.lastAdded = state.items.length - 1;
  renderList(this);
  return state.lastAdded;
}

function deleteAllItems(this: GuiObjectLike): void {
  const state = listState(this);
  state.items = [];
  state.selected.clear();
  state.focus = -1;
  state.lastAdded = -1;
  renderList(this);
}

function deleteByPos(this: GuiObjectLike, position: number): number {
  const state = listState(this);
  const index = validItem(state, position);
  if (index < 0) return 0;
  state.items.splice(index, 1);
  repositionSelection(state, index);
  renderList(this);
  return 1;
}

function getItemLabel(this: GuiObjectLike, position: number, subPosition: number): string {
  const item = listState(this).items[validItem(listState(this), position)];
  return item?.labels[integer(subPosition)] ?? "";
}

function setItemLabel(this: GuiObjectLike, position: number, label: string): void {
  setSubItem.call(this, position, 0, label);
}

function setSubItem(this: GuiObjectLike, position: number, subPosition: number, label: string): void {
  const state = listState(this);
  const index = validItem(state, position);
  const sub = integer(subPosition, -1);
  if (index < 0 || sub < 0) return;
  const labels = state.items[index].labels;
  while (labels.length <= sub) labels.push("");
  labels[sub] = String(label ?? "");
  renderList(this);
}

function setFontSize(this: GuiObjectLike, size: number): number {
  const state = listState(this);
  const normalized = Math.max(1, integer(size, state.fontSize));
  state.fontSize = normalized;
  renderList(this);
  return normalized;
}

function setColumnLabel(this: GuiObjectLike, column: number, label: string): void {
  const state = listState(this);
  const index = integer(column, -1);
  if (index < 0 || index >= state.columns.length) return;
  state.columns[index].label = String(label ?? "");
  renderList(this);
}

function scrollToItem(this: GuiObjectLike, position: number): void {
  const state = listState(this);
  const index = validItem(state, position);
  if (index < 0) return;
  const row = Array.from(this._div?.children ?? []).find(child =>
    child.getAttribute("data-classicpro-row") === String(index)) as HTMLElement | undefined;
  if (row && typeof row.scrollIntoView === "function") row.scrollIntoView({ block: "nearest" });
  else if (this._div) this._div.scrollTop = index * (state.fontSize + 4);
}

function setSelected(this: GuiObjectLike, position: number, selected: number): void {
  // The script API is used to build multi-selection programmatically; unlike a
  // plain mouse click it must not discard earlier selected rows.
  selectItem(this, integer(position), bool(selected), false);
}

function getFirstItemSelected(this: GuiObjectLike): number {
  const state = listState(this);
  for (let index = 0; index < state.items.length; index++) if (state.selected.has(index)) return index;
  return -1;
}

function getNextItemSelected(this: GuiObjectLike, lastPosition: number): number {
  const state = listState(this);
  for (let index = integer(lastPosition, -1) + 1; index < state.items.length; index++) {
    if (state.selected.has(index)) return index;
  }
  return -1;
}

function setEditText(this: GuiObjectLike, value: string): void {
  const state = editState(this);
  state.text = String(value ?? "");
  const input = syncEdit(this);
  if (input) input.value = state.text;
}

function getEditText(this: GuiObjectLike): string {
  const state = editState(this);
  const input = syncEdit(this);
  return input ? input.value : state.text;
}

function editEnter(this: GuiObjectLike): void {
  dispatch(this, "onenter");
}

function setRedraw(this: GuiObjectLike, redraw: number): void {
  const state = groupListState(this);
  const enabled = bool(redraw);
  if (state.redraw === enabled) return;
  state.redraw = enabled;
  // Wasabi's redraw flag defers layout/invalidation; it is not a visibility
  // flag. The original control lays its children out when redraw resumes.
  if (enabled) layoutGroups(this);
}

function scrollToPercent(this: GuiObjectLike, percent: number): void {
  const state = groupListState(this);
  state.scrollPercent = Math.min(100, Math.max(0, integer(percent)));
  const visibleHeight = groupListHeight(this);
  if (visibleHeight > state.maxHeight) return;
  state.scrollY = Math.trunc((state.maxHeight - visibleHeight) * state.scrollPercent / 100);
  if (state.redraw) layoutGroups(this);
}

function groupListHeight(list: GuiObjectLike): number {
  const sized = typeof (list as { getheight?: () => unknown }).getheight === "function"
    ? integer((list as { getheight: () => unknown }).getheight())
    : 0;
  return Math.max(0, sized || list._div?.clientHeight || 0);
}

function layoutGroups(list: GuiObjectLike): void {
  const state = groupListState(list);
  if (!state.redraw) return;
  if (list._div) styleControlHost(list._div);
  let offset = -state.scrollY;
  let maxWidth = 0;
  const width = typeof (list as { getwidth?: () => unknown }).getwidth === "function"
    ? Math.max(0, integer((list as { getwidth: () => unknown }).getwidth()))
    : 0;
  for (const group of state.groups) {
    const height = Math.max(0, integer(group.getheight()));
    maxWidth = Math.max(maxWidth, integer(group.getwidth()));
    group.resize(0, offset, width, height);
    offset += height;
  }
  state.maxHeight = offset + state.scrollY;
  if (list._div) {
    list._div.style.overflow = "hidden";
    list._div.style.setProperty("--classicpro-group-list-width", `${maxWidth}px`);
  }
}

async function instantiate(this: GuiObjectLike, groupId: string, count: number): Promise<Group | null> {
  const state = groupListState(this);
  const root = this._uiRoot as object | undefined;
  const materializer = root ? materializers.get(root) : undefined;
  if (!materializer) {
    throw new Error("ClassicPro GroupList.instantiate requires installClassicProControls() before the skin engine builds its UI.");
  }
  const amount = Math.max(0, integer(count));
  for (let index = 0; index < amount; index++) {
    // SkinEngineWAL.newGroup applies inheritance, recursively creates every
    // groupdef child, loads script objects, and attaches the group through the
    // GroupList addChild hook below. This must stay on the engine path rather
    // than constructing a visually empty Group shell.
    const group = await materializer.newGroup(
      Group,
      new XmlElement("group", { id: String(groupId) }),
      this,
    );
    group.init();
    state.groups.push(group);
  }
  layoutGroups(this);
  // Winamp returns the most recently created group when num_groups > 1.
  return state.groups.at(-1) ?? null;
}

function removeAllGroups(this: GuiObjectLike): void {
  const state = groupListState(this);
  for (const group of state.groups) {
    group.dispose();
    group.getDiv().remove();
    const childIndex = this._children?.indexOf(group);
    if (childIndex != null && childIndex >= 0) this._children?.splice(childIndex, 1);
  }
  state.groups = [];
  state.maxHeight = 0;
  state.scrollY = 0;
}

function addGroupChild(this: GuiObjectLike, child: Group): void {
  child.setParent(this as never);
  if (!this._children) this._children = [];
  this._children.push(child);
  this._div?.append(child.getDiv());
}

/**
 * Adds the ClassicPro controls exercised by its shipped scripts without
 * changing the upstream Webamp classes. The state is deliberately per-object,
 * so one skin's list/edit never leaks into another UIRoot.
 */
export function installClassicProControls(): void {
  if (installed) return;
  installed = true;

  // UIRoot intentionally retains the engine class, not its instance. Capture
  // the live WAL engine while it builds so dynamic GroupList groups use the
  // same parsed definitions, resource phase, and MAKI loader as the skin.
  const originalBuildUI = SkinEngineWAL.prototype.buildUI;
  SkinEngineWAL.prototype.buildUI = async function (this: GroupMaterializer) {
    materializers.set(this._uiRoot, this);
    return originalBuildUI.call(this);
  };
  const originalNewGroup = SkinEngineWAL.prototype.newGroup;
  SkinEngineWAL.prototype.newGroup = async function (
    this: GroupMaterializer,
    ...args: [typeof Group, XmlElement, unknown]
  ) {
    materializers.set(this._uiRoot, this);
    return originalNewGroup.apply(this, args);
  };
  const originalTraverseChild = SkinEngineWAL.prototype.traverseChild;
  SkinEngineWAL.prototype.traverseChild = async function (
    this: GroupMaterializer & { newGui: (Type: unknown, node: XmlElement, parent: unknown) => Promise<unknown> },
    node: XmlElement,
    parent: unknown,
  ) {
    // These are Wasabi control tags, not unknown XUI tags. Upstream's WAL
    // switch does not construct them, leaving ClassicPro's script globals
    // unbound even though their MAKI classes exist in the resolver.
    switch (node.name.toLowerCase()) {
      case "list":
      case "guilist":
        return this.newGui(GuiList, node, parent);
      case "edit":
        return this.newGui(Edit, node, parent);
      case "grouplist":
        return this.newGui(GroupList, node, parent);
      case "customobject": {
        const holder = await this.newGui(CustomObject, node, parent) as CustomObject;
        const groupId = customObjectState(holder).declaredGroupId;
        if (groupId != null) await materializeCustomObject(holder, groupId, this);
        return holder;
      }
      default:
        return originalTraverseChild.call(this, node, parent);
    }
  };

  const guiList = GuiList.prototype as unknown as Record<string, unknown>;
  const edit = Edit.prototype as unknown as Record<string, unknown>;
  const groups = GroupList.prototype as unknown as Record<string, unknown>;
  const originalGuiListSetXmlAttr = GuiList.prototype.setXmlAttr;
  const originalEditSetXmlAttr = Edit.prototype.setXmlAttr;
  const originalEditDispose = Edit.prototype.dispose;
  edit.dispose = function (this: GuiObjectLike) {
    const state = editState(this);
    if (state.idleTimer !== null) clearTimeout(state.idleTimer);
    state.idleTimer = null;
    return originalEditDispose.call(this);
  };

  guiList.setXmlAttr = function (key: string, value: string) { return listSetXmlAttr.call(this as GuiObjectLike, originalGuiListSetXmlAttr, key, value); };
  guiList.additem = addItem;
  guiList.getnumitems = function (this: GuiObjectLike) { return listState(this).items.length; };
  guiList.getitemcount = guiList.getnumitems;
  guiList.getlastaddeditempos = function (this: GuiObjectLike) { return listState(this).lastAdded; };
  guiList.deleteallitems = deleteAllItems;
  guiList.deletebypos = deleteByPos;
  guiList.getitemlabel = getItemLabel;
  guiList.getsubitemtext = getItemLabel;
  guiList.setitemlabel = setItemLabel;
  guiList.setsubitem = setSubItem;
  guiList.setfontsize = setFontSize;
  guiList.setcolumnlabel = setColumnLabel;
  guiList.getfontsize = function (this: GuiObjectLike) { return listState(this).fontSize; };
  guiList.scrolltoitem = scrollToItem;
  guiList.setselected = setSelected;
  guiList.getitemselected = function (this: GuiObjectLike, position: number) { return listState(this).selected.has(integer(position)) ? 1 : 0; };
  guiList.getfirstitemselected = getFirstItemSelected;
  guiList.getnextitemselected = getNextItemSelected;
  guiList.getitemfocused = function (this: GuiObjectLike) { return listState(this).focus; };
  guiList.setitemfocused = function (this: GuiObjectLike, position: number) { selectItem(this, integer(position), true, false); };
  guiList.setshowicons = function (this: GuiObjectLike, value: number) { listState(this).showIcons = bool(value); renderList(this); };
  guiList.getshowicons = function (this: GuiObjectLike) { return listState(this).showIcons ? 1 : 0; };
  guiList.seticonwidth = function (this: GuiObjectLike, value: number) { const state = listState(this); state.iconWidth = Math.max(0, integer(value)); renderList(this); return state.iconWidth; };
  guiList.seticonheight = function (this: GuiObjectLike, value: number) { const state = listState(this); state.iconHeight = Math.max(0, integer(value)); renderList(this); return state.iconHeight; };
  guiList.setitemicon = function (this: GuiObjectLike, position: number, icon: string) { const item = listState(this).items[validItem(listState(this), position)]; if (item) { item.icon = String(icon ?? ""); renderList(this); } };
  guiList.getitemicon = function (this: GuiObjectLike, position: number) { return listState(this).items[validItem(listState(this), position)]?.icon ?? ""; };
  guiList.setpreventmultipleselection = function (this: GuiObjectLike, value: number) { const state = listState(this); state.preventMultiple = bool(value); if (state.preventMultiple && state.selected.size > 1) selectItem(this, getFirstItemSelected.call(this), true, true); return state.preventMultiple ? 1 : 0; };
  guiList.getpreventmultipleselection = function (this: GuiObjectLike) { return listState(this).preventMultiple ? 1 : 0; };

  edit.setXmlAttr = function (key: string, value: string) { return editSetXmlAttr.call(this as GuiObjectLike, originalEditSetXmlAttr, key, value); };
  edit.settext = setEditText;
  edit.gettext = getEditText;
  edit.setfocus = function (this: GuiObjectLike) { syncEdit(this)?.focus(); };
  edit.selectall = function (this: GuiObjectLike) { syncEdit(this)?.select(); };
  edit.enter = editEnter;
  edit.setautoenter = function (this: GuiObjectLike, value: boolean) { editState(this).autoEnter = bool(value); };
  edit.getautoenter = function (this: GuiObjectLike) { return editState(this).autoEnter ? 1 : 0; };
  edit.setidleenabled = function (this: GuiObjectLike, value: boolean) { editState(this).idleEnabled = bool(value); };
  edit.getidleenabled = function (this: GuiObjectLike) { return editState(this).idleEnabled ? 1 : 0; };

  groups.instantiate = instantiate;
  // SkinEngineWAL.newGroup calls parent.addChild(). GroupList is a GuiObj, so
  // provide the native GroupList equivalent rather than bypassing the engine.
  groups.addChild = addGroupChild;
  groups.getnumitems = function (this: GuiObjectLike) { return groupListState(this).groups.length; };
  groups.enumitem = function (this: GuiObjectLike, index: number) { return groupListState(this).groups[integer(index)] ?? null; };
  groups.removeall = removeAllGroups;
  groups.scrolltopercent = scrollToPercent;
  groups.setredraw = setRedraw;
}
