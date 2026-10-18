#!/usr/bin/env node
import { openSync, readSync, closeSync, statSync, createReadStream } from "node:fs";
import { createHash } from "node:crypto";
import { basename, resolve } from "node:path";

function usage() {
  console.error("usage: node scripts/verify-installer-rea.mjs <installer.exe> [--json]");
  process.exit(2);
}

const args = process.argv.slice(2);
if (args.length === 0) usage();
const installerPath = resolve(args[0]);
const emitJson = args.includes("--json");

let stat;
try {
  stat = statSync(installerPath);
} catch {
  console.error(`installer file missing or unreadable: ${installerPath}`);
  process.exit(1);
}

const fd = openSync(installerPath, "r");
const buffer = Buffer.alloc(1024);
readSync(fd, buffer, 0, 1024, 0);

// 1. Verify DOS Header
if (buffer.readUInt16LE(0) !== 0x5a4d) { // "MZ"
  closeSync(fd);
  console.error("invalid PE DOS header (missing MZ magic)");
  process.exit(1);
}

// 2. PE Header Offset
const peOffset = buffer.readUInt32LE(0x3c);
const peHeader = Buffer.alloc(24);
readSync(fd, peHeader, 0, 24, peOffset);

if (peHeader.readUInt32LE(0) !== 0x00004550) { // "PE\0\0"
  closeSync(fd);
  console.error("invalid PE signature");
  process.exit(1);
}

const machine = peHeader.readUInt16LE(4);
const isX86 = machine === 0x014c; // NSIS installers use 32-bit bootstrap
const isX64 = machine === 0x8664;

if (!isX86 && !isX64) {
  closeSync(fd);
  console.error(`unsupported PE machine type: 0x${machine.toString(16)}`);
  process.exit(1);
}

// 3. Scan for Nullsoft signature
let hasNsisMarker = false;
const scanSize = Math.min(stat.size, 1024 * 1024);
const scanBuffer = Buffer.alloc(scanSize);
readSync(fd, scanBuffer, 0, scanSize, 0);
if (scanBuffer.includes("NullsoftInst")) {
  hasNsisMarker = true;
}
closeSync(fd);

// 4. Compute full file SHA-256
const hash = createHash("sha256");
const stream = createReadStream(installerPath);

await new Promise((res, rej) => {
  stream.on("data", (chunk) => hash.update(chunk));
  stream.on("end", res);
  stream.on("error", rej);
});

const sha256 = hash.digest("hex").toUpperCase();

const dossier = {
  evidence_id: `rea-winget-${Date.now()}`,
  subject: {
    name: basename(installerPath),
    local_path: installerPath,
    bytes: stat.size,
    sha256
  },
  pe: {
    machine: isX86 ? "IMAGE_FILE_MACHINE_I386" : "IMAGE_FILE_MACHINE_AMD64",
    is_nsis: hasNsisMarker
  },
  status: "verified"
};

if (emitJson) {
  console.log(JSON.stringify(dossier, null, 2));
} else {
  console.log(`REA verification OK for ${basename(installerPath)}`);
  console.log(`  SHA-256:   ${sha256}`);
  console.log(`  Size:      ${stat.size} bytes`);
  console.log(`  PE Target: ${dossier.pe.machine}`);
  console.log(`  NSIS Stub: ${hasNsisMarker ? "yes" : "no"}`);
}
