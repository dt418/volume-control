import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { writeFileSync, unlinkSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";

const scriptPath = resolve("scripts/verify-installer-rea.mjs");

test("verify-installer-rea rejects missing file", () => {
  assert.throws(
    () => execFileSync(process.execPath, [scriptPath, "nonexistent-setup.exe"], { stdio: "pipe" }),
    /installer file missing or unreadable/i
  );
});

test("verify-installer-rea rejects non-PE file", () => {
  const dummy = resolve("output/dummy-installer.exe");
  mkdirSync("output", { recursive: true });
  writeFileSync(dummy, "NOT A PE FILE");
  try {
    assert.throws(
      () => execFileSync(process.execPath, [scriptPath, dummy], { stdio: "pipe" }),
      /invalid PE DOS header/i
    );
  } finally {
    try { unlinkSync(dummy); } catch {}
  }
});

test("verify-installer-rea accepts valid PE binary with JSON output", () => {
  const out = execFileSync(process.execPath, [scriptPath, process.execPath, "--json"], { encoding: "utf8" });
  const parsed = JSON.parse(out);
  assert.equal(parsed.status, "verified");
  assert.equal(parsed.subject.name, "node.exe");
  assert.equal(typeof parsed.subject.sha256, "string");
  assert.equal(parsed.subject.sha256.length, 64);
  assert.equal(parsed.pe.machine, "IMAGE_FILE_MACHINE_AMD64");
});
