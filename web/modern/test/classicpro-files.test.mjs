import assert from "node:assert/strict";
import { test } from "node:test";
import { Buffer } from "node:buffer";
import { fileURLToPath } from "node:url";
import path from "node:path";
import esbuild from "esbuild";

const directory = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

async function loadFixture() {
  const result = await esbuild.build({
    bundle: true,
    format: "esm",
    platform: "node",
    write: false,
    stdin: {
      loader: "ts",
      resolveDir: directory,
      sourcefile: "classicpro-files-fixture.ts",
      contents: `
        import { installClassicProFileApi } from "./src/classicpro-files.ts";
        import File from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/File.ts";
        import XmlDoc from "../../native/webamp/packages/webamp-modern/src/skin/makiClasses/XmlDoc.ts";
        export { installClassicProFileApi, File, XmlDoc };
      `,
    },
  });
  const source = Buffer.from(result.outputFiles[0].contents).toString("base64");
  return import(`data:text/javascript;base64,${source}`);
}

test("ClassicPro File and XmlDoc use only supplied resources and dispatch MAKI callbacks", async () => {
  const { File, XmlDoc, installClassicProFileApi } = await loadFixture();
  const encoder = new TextEncoder();
  const resources = new Map([
    ["config.xml", encoder.encode('<ClassicPro><TextSettings><Style id="normal" value="A"/></TextSettings><About:Skin name="B"/></ClassicPro>')],
    ["broken.xml", encoder.encode("<ClassicPro>")],
  ]);
  const calls = [];
  const root = { vm: { dispatch(object, event, args) { calls.push({ object, event, args }); } } };
  installClassicProFileApi((_root, resourcePath) => resources.get(resourcePath) ?? null);

  const file = new File(root);
  file.load("config.xml");
  assert.equal(file.exists(), true);
  assert.equal(file.getsize(), resources.get("config.xml").byteLength);
  file.load("https://example.invalid/not-allowed.xml");
  assert.equal(file.exists(), false);
  assert.equal(file.getsize(), 0);

  const doc = new XmlDoc(root);
  doc.load("config.xml");
  doc.parser_addcallback("ClassicPro/TextSettings*");
  doc.parser_addcallback("ClassicPro/About:Skin*");
  doc.parser_start();
  assert.deepEqual(calls.map(call => call.event), [
    "parser_oncallback", "parser_oncallback", "parser_onclosecallback",
    "parser_onclosecallback", "parser_oncallback", "parser_onclosecallback",
  ]);
  assert.equal(calls[1].args[0].value, "CLASSICPRO/TEXTSETTINGS/STYLE");
  assert.equal(calls[1].args[1].value, "Style");
  assert.deepEqual(calls[1].args[2].value._list, ["id", "value"]);
  assert.deepEqual(calls[1].args[3].value._list, ["normal", "A"]);
  assert.equal(calls[4].args[0].value, "CLASSICPRO/ABOUT:SKIN");

  doc.parser_destroy();
  calls.length = 0;
  doc.parser_start();
  assert.equal(calls.length, 0);

  doc.load("broken.xml");
  assert.throws(() => doc.parser_start(), /Missing end tag/);
  assert.equal(calls.at(-1).event, "parser_onerror");
  assert.equal(calls.at(-1).args[0].value, "broken.xml");
});
