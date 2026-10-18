import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { writeFileSync, unlinkSync, mkdirSync, existsSync } from "node:fs";
import { resolve } from "node:path";

const scriptPath = resolve("scripts/verify-installer-rea.mjs");

test("verify-installer-rea rejects missing file", () => {
  assert.throws(
    () => execFileSync(process.execPath, [scriptPath, "nonexistent-setup.exe"], { stdio: "pipe" }),
    /Target installer file does not exist|installer file missing or unreadable/i
  );
});

test("verify-installer-rea rejects non-PE file", () => {
  const dummy = resolve("output/dummy-installer.exe");
  mkdirSync("output", { recursive: true });
  // Create dummy > 1024 bytes with non-PE content
  writeFileSync(dummy, Buffer.alloc(2048, 0x41));
  try {
    assert.throws(
      () => execFileSync(process.execPath, [scriptPath, dummy], { stdio: "pipe" }),
      /Invalid DOS header magic|invalid PE DOS header/i
    );
  } finally {
    try { unlinkSync(dummy); } catch {}
  }
});

test("verify-installer-rea inspects valid VolumeControl NSIS installer when present", (t) => {
  const candidates = [
    resolve("output/VolumeControl_0.1.7_x64-setup.exe"),
    "C:/Users/Thanh/AppData/Local/Temp/winget-017/VolumeControl_0.1.7_x64-setup.exe",
    "C:/Users/Thanh/AppData/Local/Temp/winget-v015/VolumeControl_0.1.5_x64-setup.exe"
  ];
  const target = candidates.find((c) => existsSync(c));
  if (!target) {
    t.skip("No installer found in local cache; skipping positive NSIS inspection");
    return;
  }
  const out = execFileSync(process.execPath, [scriptPath, target, "--json"], { encoding: "utf8" });
  const parsed = JSON.parse(out);
  assert.equal(parsed.dosHeader.magic, "MZ");
  assert.equal(parsed.peHeader.machine, "0x14c");
  assert.equal(parsed.nsis.marker, "NullsoftInst");
  assert.equal(parsed.nsis.magicBytesValid, true);
  assert.equal(parsed.wingetCorrelation.installerType, "nullsoft");
  assert.equal(parsed.wingetCorrelation.architecture, "x64");
  assert.equal(parsed.wingetCorrelation.scope, "user");
  assert.equal(typeof parsed.sha256, "string");
  assert.equal(parsed.sha256.length, 64);
});
