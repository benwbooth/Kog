import { build } from "esbuild";
import { execFile } from "node:child_process";
import {
  cp,
  mkdir,
  readFile,
  readdir,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";

const projectDir = path.dirname(fileURLToPath(import.meta.url));
const repositoryDir = path.resolve(projectDir, "../..");
const distDir = path.join(projectDir, "dist");
const webampDir = path.join(repositoryDir, "native/webamp");
const interpreterPath = path.join(
  webampDir,
  "packages/webamp-modern/src/maki/interpreter.ts",
);
const execFileAsync = promisify(execFile);

const makiInstructionBudgetPlugin = {
  name: "kog-maki-instruction-budget",
  setup(buildContext) {
    buildContext.onLoad({ filter: /[/\\]maki[/\\]interpreter\.ts$/ }, async (args) => {
      let contents = await readFile(args.path, "utf8");
      if (path.resolve(args.path) !== interpreterPath) {
        return { contents, loader: "ts" };
      }
      const original = "    let ip = start;\n    while (ip < this.commands.length) {\n      const command = this.commands[ip];";
      const bounded = `    let ip = start;
    // Kog treats skin scripts as untrusted presentation code. A corrupt or
    // hostile backwards jump must not monopolize Qt's renderer process.
    let remainingInstructions = 100_000;
    while (ip < this.commands.length) {
      if (--remainingInstructions < 0) {
        throw new Error(\`MAKI instruction budget exceeded in \${this.maki_id}\`);
      }
      const command = this.commands[ip];`;
      if (!contents.includes(original)) {
        throw new Error("Pinned Webamp MAKI interpreter changed; refusing to build without its instruction budget");
      }
      contents = contents.replace(original, bounded);
      return { contents, loader: "ts" };
    });
  },
};

function escapeXml(value) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

async function firstLicenseFile(packageDir) {
  const entries = await readdir(packageDir);
  const name = entries.find((entry) => /^(license|copying)(\.|$)/i.test(entry));
  return name ? path.join(packageDir, name) : null;
}

function packageDirectoryForInput(input) {
  const marker = "node_modules/";
  const markerIndex = input.lastIndexOf(marker);
  if (markerIndex < 0) return null;
  const remainder = input.slice(markerIndex + marker.length).split("/");
  const packageParts = remainder[0].startsWith("@")
    ? remainder.slice(0, 2)
    : remainder.slice(0, 1);
  return path.join(projectDir, "node_modules", ...packageParts);
}

async function generateNotices(metafile) {
  const packageDirs = new Set(
    Object.keys(metafile.inputs)
      .map(packageDirectoryForInput)
      .filter(Boolean),
  );
  const sections = [];

  const webampLicense = await readFile(path.join(webampDir, "LICENSE.txt"), "utf8");
  const { stdout: webampRevisionOutput } = await execFileAsync(
    "git",
    ["-C", webampDir, "rev-parse", "HEAD"],
    { encoding: "utf8", timeout: 5_000 },
  );
  const webampRevision = webampRevisionOutput.trim();
  if (!/^[0-9a-f]{40}$/.test(webampRevision)) {
    throw new Error(`Invalid pinned Webamp revision: ${webampRevision}`);
  }
  sections.push(
    [
      "Webamp / Webamp Modern renderer source",
      `Pinned revision: ${webampRevision}`,
      "License: MIT",
      "",
      webampLicense.trim(),
    ].join("\n"),
  );

  for (const packageDir of [...packageDirs].sort()) {
    const packageJson = JSON.parse(
      await readFile(path.join(packageDir, "package.json"), "utf8"),
    );
    const licensePath = await firstLicenseFile(packageDir);
    const licenseText = licensePath
      ? (await readFile(licensePath, "utf8")).trim()
      : "No license text was shipped in this package; see its package metadata.";
    sections.push(
      [
        `${packageJson.name} ${packageJson.version}`,
        `License: ${packageJson.license || "not declared"}`,
        "",
        licenseText,
      ].join("\n"),
    );
  }

  const separator = `\n\n${"=".repeat(78)}\n\n`;
  await writeFile(
    path.join(distDir, "THIRD_PARTY_NOTICES.txt"),
    `${sections.join(separator)}\n`,
  );
}

await rm(distDir, { recursive: true, force: true });
await mkdir(distDir, { recursive: true });

const result = await build({
  absWorkingDir: projectDir,
  entryPoints: ["src/runtime.ts"],
  outfile: "dist/runtime.js",
  bundle: true,
  platform: "browser",
  nodePaths: [path.join(projectDir, "node_modules")],
  target: ["chrome100"],
  format: "iife",
  minify: true,
  // Webamp derives several custom-element tags from constructor.name.
  keepNames: true,
  // Preserve embedded shader/text bytes as escaped strings rather than
  // multiline literals with upstream trailing spaces in the generated bundle.
  supported: { "template-literal": false },
  legalComments: "none",
  metafile: true,
  loader: {
    ".xml": "text",
  },
  define: {
    "process.env.NODE_ENV": '"production"',
  },
  plugins: [makiInstructionBudgetPlugin],
});

await cp(path.join(projectDir, "index.html"), path.join(distDir, "index.html"));
await generateNotices(result.metafile);

const qrcFiles = [
  "index.html",
  "runtime.js",
  "runtime.css",
  "THIRD_PARTY_NOTICES.txt",
];
for (const filename of qrcFiles) {
  await stat(path.join(distDir, filename));
}
const qrc = `<!DOCTYPE RCC>
<RCC version="1.0">
  <qresource prefix="/kog/modern">
${qrcFiles
  .map(
    (filename) =>
      `    <file alias="${escapeXml(filename)}">dist/${escapeXml(filename)}</file>`,
  )
  .join("\n")}
  </qresource>
</RCC>
`;
await writeFile(path.join(projectDir, "runtime.qrc"), qrc);
