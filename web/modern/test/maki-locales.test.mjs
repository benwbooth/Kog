import assert from "node:assert/strict";
import { test } from "node:test";
import { getMakiString, lookupMakiString, registerMakiStrings, renderedMakiString } from "../src/maki-locales.js";

test("Wasabi string references preserve missing values, empty entries, exact table names and root isolation", () => {
  const root = {};
  registerMakiStrings(root, "Skin", [[0, "zero"], [12, "translated"], [13, ""]]);
  assert.equal(lookupMakiString(root, "@Skin#12"), "translated");
  assert.equal(lookupMakiString(root, "@Skin# +12suffix"), "translated");
  assert.equal(lookupMakiString(root, "@Skin#nonnumeric"), "zero");
  assert.equal(lookupMakiString(root, "@Skin#13"), "");
  for (const value of ["@Skin#14", "@skin#12", "literal", "@Skin", "prefix@Skin#12"]) {
    assert.equal(lookupMakiString(root, value), value);
  }
  assert.equal(lookupMakiString({}, "@Skin#12"), "@Skin#12");
  registerMakiStrings(root, "Skin", [[12, "updated"]]);
  assert.equal(getMakiString(root, "Skin", 0), "zero");
  assert.equal(lookupMakiString(root, "@Skin#12"), "updated");
  const longTable = "x".repeat(127);
  registerMakiStrings(root, longTable, [[12, "too long"]]);
  assert.equal(lookupMakiString(root, `@${longTable}#12`), `@${longTable}#12`);
  for (const mode of [undefined, "0", "1"]) {
    assert.equal(renderedMakiString({ _uiRoot: root, _translate: mode }, "@Skin#12"), "@Skin#12");
  }
  assert.equal(renderedMakiString({ _uiRoot: root, _translate: "2" }, "@Skin#12"), "updated");
});
