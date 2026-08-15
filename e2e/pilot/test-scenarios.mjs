import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import { resolve } from "node:path";

const repositoryRoot = resolve(import.meta.dirname, "../..");
const scenarioRoot = resolve(import.meta.dirname, "scenarios");
const packageJson = JSON.parse(await readFile(resolve(repositoryRoot, "e2e/tauri/package.json"), "utf8"));

assert.equal(packageJson.scripts["test:pilot"], "node ../pilot/run-pilot.mjs");
assert.equal(packageJson.scripts["test:pilot-contract"], "node ../pilot/test-scenarios.mjs");

const required = {
  mixer: ["eval", "assert-text", "assert-visible", "click"],
  settings: ["eval", "wait", "fill", "click", "assert-visible"],
  help: ["eval", "wait", "fill", "assert-visible"],
  recovery: ["assert-visible", "assert-text", "eval"],
};

for (const [name, actions] of Object.entries(required)) {
  const path = resolve(scenarioRoot, `${name}.toml`);
  await access(path);
  const source = await readFile(path, "utf8");
  assert.match(source, /^\[scenario\]/m, `${name} scenario must declare metadata`);
  assert.match(source, /^\[\[step\]\]/m, `${name} scenario must declare steps`);
  assert.match(source, /action\s*=\s*"screenshot"/, `${name} scenario must capture a screenshot`);
  for (const action of actions) {
    assert.match(source, new RegExp(`action\\s*=\\s*"${action}"`), `${name} scenario is missing ${action}`);
  }
  assert.doesNotMatch(source, /action\s*=\s*"press"/, `${name} must not claim global shortcut coverage`);
}

const readme = await readFile(resolve(import.meta.dirname, "README.md"), "utf8");
assert.match(readme, /cargo install tauri-pilot-cli/);
assert.match(readme, /WebdriverIO is\s+the release gate/);
assert.match(readme, /nonzero|unavailable|never silently/i);
console.log("Tauri Pilot scenario contract passed");
