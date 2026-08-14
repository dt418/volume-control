import assert from "node:assert/strict";
import { test } from "node:test";
import { clearTimings, recordTiming, sanitizeArtifactName, timingReport } from "./artifacts.ts";

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
