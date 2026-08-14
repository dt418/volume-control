import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import { resolve } from "node:path";

const repositoryRoot = resolve(import.meta.dirname, "../..");
const cargoToml = await readFile(resolve(repositoryRoot, "src-tauri/Cargo.toml"), "utf8");
const libRs = await readFile(resolve(repositoryRoot, "src-tauri/src/lib.rs"), "utf8");
const defaultCapability = await readFile(resolve(repositoryRoot, "src-tauri/capabilities/default.json"), "utf8");

assert.match(cargoToml, /e2e-wdio\s*=\s*\["dep:tauri-plugin-wdio"/);
assert.match(cargoToml, /e2e-pilot\s*=\s*\["dep:tauri-plugin-pilot"/);
assert.match(cargoToml, /tauri-plugin-pilot\s*=\s*\{[^\n]*default-features\s*=\s*false[^\n]*optional\s*=\s*true/);
assert.match(libRs, /cfg!\(debug_assertions\)/);
assert.match(libRs, /VOLUMECTL_E2E_DEBUG/);
assert.doesNotMatch(defaultCapability, /wdio|pilot/);
await assert.rejects(access(resolve(repositoryRoot, "src-tauri/capabilities/e2e-wdio.json")));
await assert.rejects(access(resolve(repositoryRoot, "src-tauri/capabilities/e2e-pilot.json")));
console.log("Production exclusion contract passed");
