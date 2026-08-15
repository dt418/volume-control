import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  assertE2eEvidence,
  clearTimings,
  recordTiming,
  sanitizeArtifactName,
  timingReport,
  writeE2eManifest,
  type E2eManifest,
} from "./artifacts.ts";

test("sanitizes artifact names and keeps a bounded stable path segment", () => {
  assert.equal(sanitizeArtifactName(" Settings / bootstrap failure "), "Settings-bootstrap-failure");
  assert.equal(sanitizeArtifactName("///"), "unnamed");
});

test("reports deterministic p50 and p95 timing values", () => {
  clearTimings();
  for (const value of [10, 20, 30, 40]) recordTiming("bootstrap", value);
  assert.deepEqual(timingReport(), { bootstrap: { count: 4, p50: 20, p95: 40 } });
});

test("rejects invalid timings", () => {
  assert.throws(() => recordTiming("bad", Number.NaN), /Invalid timing/);
  assert.throws(() => recordTiming("bad", -1), /Invalid timing/);
});

async function evidenceFixture(): Promise<string> {
  const outputRoot = await mkdtemp(join(tmpdir(), "volumecontrol-e2e-evidence-"));
  await mkdir(join(outputRoot, "junit"), { recursive: true });
  await writeFile(join(outputRoot, "junit", "mixer.xml"), "<testsuite></testsuite>\n");
  await writeFile(join(outputRoot, "timings.json"), "{}\n");
  return outputRoot;
}

test("rejects a run when the JUnit artifact is missing", async () => {
  const outputRoot = await mkdtemp(join(tmpdir(), "volumecontrol-e2e-missing-junit-"));
  await writeFile(join(outputRoot, "timings.json"), "{}\n");
  await writeE2eManifest(outputRoot, { results: [{ spec: "mixer.e2e.ts", status: "passed" }] });

  await assert.rejects(
    assertE2eEvidence(outputRoot, ["mixer.e2e.ts"]),
    /Missing required JUnit artifact/,
  );
});

test("rejects a manifest that omits an expected spec result", async () => {
  const outputRoot = await evidenceFixture();
  const manifest: E2eManifest = {
    results: [{ spec: "mixer.e2e.ts", status: "passed" }],
  };
  await writeE2eManifest(outputRoot, manifest);

  await assert.rejects(
    assertE2eEvidence(outputRoot, ["mixer.e2e.ts", "runtime.e2e.ts"]),
    /Manifest is missing result for expected spec: runtime\.e2e\.ts/,
  );
});
