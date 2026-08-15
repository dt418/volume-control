import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  assertE2eEvidence,
  assertConfiguredTimingBudgets,
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
  assert.deepEqual(timingReport(), { bootstrap: { count: 4, p50: 20, p95: 40, budget: 3000 } });
});

test("enforces bootstrap and IPC budgets but leaves observational timings unblocked", () => {
  clearTimings();
  recordTiming("bootstrap", 3_000);
  recordTiming("ipc", 250);
  recordTiming("render", 60_000);
  assert.doesNotThrow(() => assertConfiguredTimingBudgets());

  recordTiming("ipc", 251);
  assert.throws(() => assertConfiguredTimingBudgets(), /Timing budget exceeded for ipc/);
});

test("rejects invalid timings", () => {
  assert.throws(() => recordTiming("bad", Number.NaN), /Invalid timing/);
  assert.throws(() => recordTiming("bad", -1), /Invalid timing/);
});

async function evidenceFixture(): Promise<string> {
  const outputRoot = await mkdtemp(join(tmpdir(), "volumecontrol-e2e-evidence-"));
  await mkdir(join(outputRoot, "junit"), { recursive: true });
  await writeFile(join(outputRoot, "junit", "mixer.e2e.ts.xml"), "<testsuite></testsuite>\n");
  await writeFile(join(outputRoot, "timings.json"), "{}\n");
  return outputRoot;
}

test("rejects a run when the JUnit artifact is missing", async () => {
  const outputRoot = await mkdtemp(join(tmpdir(), "volumecontrol-e2e-missing-junit-"));
  await mkdir(join(outputRoot, "junit"), { recursive: true });
  await writeFile(join(outputRoot, "junit", "stale.e2e.ts.xml"), "<testsuite></testsuite>\n");
  await writeFile(join(outputRoot, "timings.json"), "{}\n");
  await writeE2eManifest(outputRoot, { runId: "current-run", results: [{ spec: "mixer.e2e.ts", surface: "mixer", status: "passed" }] });

  await assert.rejects(
    assertE2eEvidence(outputRoot, ["mixer.e2e.ts"]),
    /Missing required JUnit artifact/,
  );
});

test("rejects a manifest that omits an expected spec result", async () => {
  const outputRoot = await evidenceFixture();
  const manifest: E2eManifest = {
    runId: "current-run",
    results: [{ spec: "mixer.e2e.ts", surface: "mixer", status: "passed" }],
  };
  await writeE2eManifest(outputRoot, manifest);

  await assert.rejects(
    assertE2eEvidence(outputRoot, ["mixer.e2e.ts", "runtime.e2e.ts"]),
    /Manifest is missing result for expected spec: runtime\.e2e\.ts/,
  );
});

test("requires an exact current manifest entry for every requested surface", async () => {
  const outputRoot = await mkdtemp(join(tmpdir(), "volumecontrol-e2e-all-surfaces-"));
  const expectedSpecs = [
    "mixer.e2e.ts",
    "runtime.e2e.ts",
    "windows.e2e.ts",
    "recovery.e2e.ts",
    "settings.e2e.ts",
    "help.e2e.ts",
  ];
  await mkdir(join(outputRoot, "junit"), { recursive: true });
  for (const spec of expectedSpecs) {
    await writeFile(join(outputRoot, "junit", `${spec}.xml`), "<testsuite></testsuite>\n");
  }
  await writeFile(join(outputRoot, "timings.json"), "{}\n");
  await writeE2eManifest(outputRoot, {
    runId: "current-run",
    results: expectedSpecs.map((spec) => ({
      spec,
      surface: spec.replace(/\.e2e\.ts$/u, ""),
      status: "passed" as const,
    })),
  });

  await assertE2eEvidence(outputRoot, expectedSpecs, "current-run");
  await assert.rejects(
    assertE2eEvidence(outputRoot, expectedSpecs, "previous-run"),
    /Manifest run ID is stale/,
  );
});
