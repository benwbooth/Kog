import assert from "node:assert/strict";
import { test } from "node:test";
import { makiMember } from "../src/maki-members.js";
import { MakiPreferences, installBitListApi } from "../src/maki-preferences.js";

test("MAKI members preserve references and isolate objects and scripts", () => {
  const object = {}, script = [], classes = ["gui-guid"];
  const member = makiMember(object, script, "Item", 2, classes);
  assert.deepEqual(member, { type: "INT", value: 0 });
  member.value = 17;
  assert.equal(makiMember(object, script, "ITEM", 2, classes), member);
  assert.equal(makiMember(object, [], "item", 2, classes).value, 0);
  assert.equal(makiMember({}, script, "item", 2, classes).value, 0);
  assert.deepEqual(makiMember(object, script, "title", 6, classes), { type: "STRING", value: "" });
  assert.deepEqual(makiMember(object, script, "target", 256, classes), { type: "OBJECT", value: null, guid: "gui-guid" });
  assert.throws(() => makiMember(null, script, "x", 2, classes));
  assert.throws(() => makiMember(object, script, "x", 255, classes));
  assert.throws(() => makiMember(object, script, "x", 257, classes));
});

test("public preferences preserve defaults, case-insensitive keys and stored values", () => {
  const data = new Map();
  const storage = { getItem: key => data.get(key) ?? null, setItem: (key, value) => data.set(key, value) };
  const prefs = new MakiPreferences(storage);
  assert.equal(prefs.getInt("missing", 42), 42);
  assert.equal(prefs.getString("missing", "default"), "default");
  assert.equal(data.size, 0);
  prefs.setInt("COUNT", 7);
  assert.equal(new MakiPreferences(storage).getInt("count", 0), 7);
  prefs.setString("text", "");
  assert.equal(prefs.getString("text", "default"), "");
  prefs.setString("__proto__", "safe");
  assert.equal(prefs.getString("__proto__", ""), "safe");
});

test("BitList has bounded resize and native out-of-range behavior", () => {
  class Bits { _items = []; }
  installBitListApi(Bits);
  const bits = new Bits();
  assert.equal(bits.getitem(0), false);
  bits.setitem(3, true);
  assert.equal(bits._items.length, 0);
  bits.setsize(4);
  bits.setitem(3, true);
  assert.equal(bits.getitem(3), true);
  bits.setsize(2);
  assert.equal(bits.getitem(3), false);
  bits.setsize(4);
  assert.equal(bits.getitem(3), false);
  assert.throws(() => bits.setsize(-1));
  assert.throws(() => bits.setsize(1048577));
});
