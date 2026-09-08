// Wasabi LocalesManager::GetString/lookupString. Tables belong to a skin root,
// not to the renderer process, and their names are case-sensitive.
const tablesByRoot = new WeakMap();

export function registerMakiStrings(root, table, entries) {
  let tables = tablesByRoot.get(root);
  if (!tables) tablesByRoot.set(root, tables = new Map());
  let strings = tables.get(table);
  if (!strings) tables.set(table, strings = new Map());
  for (const [id, value] of entries) strings.set(id, value);
}

export function getMakiString(root, table, id) {
  const number = Number(id);
  if (!root || !Number.isSafeInteger(number) || number < 0) return undefined;
  return tablesByRoot.get(root)?.get(String(table ?? ""))?.get(number);
}

export function lookupMakiString(root, value) {
  if (typeof value !== "string" || !value.startsWith("@")) return value;
  const pound = value.indexOf("#");
  if (pound < 1 || pound >= 128) return value;
  // WTOI accepts a signed decimal prefix, and returns zero without one.
  const id = Number.parseInt(value.slice(pound + 1), 10) || 0;
  return getMakiString(root, value.slice(1, pound), id >>> 0) ?? value;
}

export function renderedMakiString(object, value) {
  return Number.parseInt(object._translate, 10) === 2
    ? lookupMakiString(object._uiRoot, value)
    : value;
}
