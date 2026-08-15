import assert from "node:assert/strict";
import { test } from "node:test";
import { selectors } from "./selectors.ts";

test("uses stable surface roots and accessible primary controls", () => {
  assert.equal(selectors.surfaceRoot("mixer"), '[data-surface="mixer"]');
  assert.equal(selectors.surfaceTitle("settings"), '[data-surface="settings"] h1');
  assert.equal(selectors.header("mixer"), '[data-surface="mixer"] [data-testid="surface-header"]');
  assert.equal(selectors.mixer.search, '[data-surface="mixer"] input[placeholder="Search apps…"]');
  assert.equal(selectors.mixer.systemVolume, '[aria-label="System output volume"]');
  assert.equal(selectors.settings.status, '[aria-live="polite"]');
});
