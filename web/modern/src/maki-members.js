// MAKI OPCODE_UMV (0x68): persistent user members belong to an object AND
// a script, not to an individual event interpreter or to JavaScript properties.
// Reference: Wasabi api/script/vcpu.cpp OPCODE_UMV and scriptobji.cpp getMember.
const objectMembers = new WeakMap();
const embeddedObjectIds = new WeakMap();

export function setMakiEmbeddedObject(object, id) {
  if (id) embeddedObjectIds.set(object, String(id));
  else embeddedObjectIds.delete(object);
}

export function getMakiEmbeddedObject(object) {
  const id = embeddedObjectIds.get(object);
  const child = id ? object._findobject?.(id) : null;
  return child && child !== object ? child : null;
}

// Wasabi ScriptObjectI::vcpu_getInterface delegates unsupported interfaces to
// Group::script_cast, which queries the group's declared embed_xui child.
// Preserve interfaces the wrapper already implements, and never invent methods.
export function makiInterface(object, ExpectedClass) {
  let current = object;
  const visited = new Set();
  while (current && embeddedObjectIds.has(current)) {
    if (current instanceof ExpectedClass) return current;
    if (visited.has(current) || visited.size >= 64) throw new Error("Cyclic MAKI embedded interface");
    visited.add(current);
    const child = current._findobject?.(embeddedObjectIds.get(current));
    if (!child || child === current) break;
    if (child instanceof ExpectedClass) return child;
    current = child;
  }
  return object;
}

export function makiMember(object, script, name, typeCode, classes) {
  if (!object || (typeof object !== "object" && typeof object !== "function")) throw new Error("MAKI member requires an object");
  if (typeof name !== "string" || !name || name.length > 4096) throw new Error("Invalid MAKI member name");
  let scripts = objectMembers.get(object);
  if (!scripts) objectMembers.set(object, scripts = new WeakMap());
  let members = scripts.get(script);
  if (!members) scripts.set(script, members = new Map());
  const key = name.toLowerCase();
  if (members.has(key)) return members.get(key);
  if (members.size >= 4096) throw new Error("MAKI object member limit exceeded");
  const types = { 2: "INT", 3: "FLOAT", 4: "DOUBLE", 5: "BOOLEAN", 6: "STRING", 7: "OBJECT" };
  const type = typeCode >= 0x100 ? "OBJECT" : types[typeCode];
  if (!type) throw new Error(`Invalid MAKI member type ${typeCode}`);
  const variable = { type, value: type === "STRING" ? "" : type === "OBJECT" ? null : 0 };
  if (typeCode >= 0x100) {
    const guid = classes[typeCode - 0x100];
    if (typeof guid !== "string") throw new Error(`Invalid MAKI member class ${typeCode}`);
    variable.guid = guid;
  }
  members.set(key, variable);
  return variable;
}
