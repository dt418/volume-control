import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const packageJson = JSON.parse(await readFile(resolve(packageRoot, "package.json"), "utf8"));
const frontendJson = JSON.parse(await readFile(resolve(packageRoot, "../../frontend/package.json"), "utf8"));

assert.equal(packageJson.scripts["test:e2e"], "wdio run wdio.conf.ts");
assert.equal(packageJson.scripts["test:contract"], "node --test test/package-contract.mjs");
assert.ok(await readFile(resolve(packageRoot, "wdio.conf.ts"), "utf8"));
assert.equal(frontendJson.dependencies?.webdriverio, undefined);
assert.equal(frontendJson.devDependencies?.webdriverio, undefined);
assert.equal(frontendJson.devDependencies?.["@wdio/tauri-service"], undefined);
