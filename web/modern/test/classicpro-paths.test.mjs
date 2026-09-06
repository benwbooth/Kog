import assert from "node:assert/strict";
import test from "node:test";
import { CPRO_ROOT, classicProPath, engineIncludes, prepareClassicProXml } from "../src/classicpro.js";

test("ClassicPro virtual paths recognize both Winamp path tokens", () => {
  for (const path of [
    "@COLORTHEMESPATH@/../../Plugins/classicPro/engine/load.xml",
    "@WINAMPPATH@\\Plugins\\ClassicPro\\engine\\LOAD.XML",
    "Plugins/ClassicPro/engine/load.xml",
    CPRO_ROOT + "load.xml",
  ]) assert.equal(classicProPath(path), CPRO_ROOT + "load.xml");
});

test("ClassicPro virtual paths never accept external URLs or escaped engine paths", () => {
  for (const path of [
    "https://example.com/Plugins/classicPro/engine/load.xml",
    "file:///Plugins/classicPro/engine/load.xml",
    "@WINAMPPATH@/Plugins/classicPro/engine/../secret.xml",
    "@WINAMPPATH@/Plugins/classicPro/engine/%2e%2e/secret.xml",
    CPRO_ROOT + "../../secret", CPRO_ROOT + "load.xml?extra", "skin.xml",
  ]) assert.equal(classicProPath(path), null, path);
});

test("engine includes resolve parent directories only within the bundled engine", () => {
  assert.deepEqual(engineIncludes("..\\Data\\NowPlaying\\NowPlaying.xml", CPRO_ROOT + "widgets/load/", {}),
    [CPRO_ROOT + "widgets/data/nowplaying/nowplaying.xml"]);
  assert.throws(() => engineIncludes("../../secret.xml", CPRO_ROOT + "widgets/", {}), /escapes/);
  assert.equal(engineIncludes("xml/player.xml", null, {}), null);
});

test("wildcards include sorted XML from that directory only", () => {
  const files = { "widgets/load/b.xml": "", "widgets/load/a.xml": "", "widgets/load/v2/c.xml": "", "widgets/load/a.png": "" };
  assert.deepEqual(engineIncludes("widgets/load/*.xml", CPRO_ROOT, files),
    [CPRO_ROOT + "widgets/load/a.xml", CPRO_ROOT + "widgets/load/b.xml"]);
  assert.throws(() => engineIncludes("widgets/**/*.xml", CPRO_ROOT, files), /wildcard/);
});

test("Wasabi XUI colon tags survive engine include rewriting", () => {
  const xml = '<groupdef id="main"><Centro:SUI id="cpro.sui"/><include file="child.xml"/><text text="A &amp; B"/></groupdef>';
  const result = prepareClassicProXml(xml, CPRO_ROOT + "one/xml/player.xml", {});
  assert.match(result, /<Centro:SUI id="cpro.sui">/);
  assert.match(result, /file="__kog_classicpro__\/one\/xml\/child.xml"/);
  assert.match(result, /text="A &amp; B"/);
  assert.equal(prepareClassicProXml('<layout id="normal"/>', "skin.xml", {}), '<layout id="normal"/>');
});
