import assert from "node:assert/strict";
import { access } from "node:fs/promises";
import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { createAppFixture } from "./app-fixture.ts";

test("creates an isolated config directory and idempotent cleanup", async () => {
  const root = await mkdtemp(join(tmpdir(), "volumecontrol-e2e-"));
  const fixture = await createAppFixture({ outputRoot: root, binary: "target/debug/test-binary" });

  assert.ok(fixture.configDir.startsWith(root));
  assert.ok(fixture.outputDir.startsWith(root));
  assert.equal(fixture.binary.endsWith("target\\debug\\test-binary") || fixture.binary.endsWith("target/debug/test-binary"), true);

  await access(fixture.configDir);
  await fixture.cleanup();
  await fixture.cleanup();
  await assert.rejects(access(fixture.configDir));
});
