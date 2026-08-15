import { mkdir, readdir, readFile, writeFile } from "node:fs/promises";
import { mkdirSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";

type BrowserLike = Record<string, unknown>;
const timings = new Map<string, number[]>();

/**
 * Only the two deterministic startup/IPC paths have release-gate budgets.
 * Render, screenshot, and audio timings remain observational evidence because
 * their host scheduling is intentionally outside the WDIO contract.
 */
export const DEFAULT_TIMING_BUDGETS: Readonly<Record<string, number>> = {
  bootstrap: 3_000,
  ipc: 250,
};

const budgetAliases: Readonly<Record<string, string>> = {
  "bootstrap-to-ready": "bootstrap",
  "mixer-to-ready": "bootstrap",
};

export interface E2eResult {
  spec: string;
  status: "passed" | "failed" | "skipped";
  surface?: string;
  durationMs?: number;
  screenshot?: string;
  snapshot?: string;
  frontendLog?: string;
  backendLog?: string;
}

export interface E2eManifest {
  schemaVersion?: 1;
  runId?: string;
  platform?: string;
  provider?: string;
  results: E2eResult[];
  [key: string]: unknown;
}

export function sanitizeArtifactName(value: string): string {
  return value
    .trim()
    .replace(/[^a-zA-Z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 120) || "unnamed";
}

export function recordTiming(name: string, milliseconds: number): void {
  if (!Number.isFinite(milliseconds) || milliseconds < 0) {
    throw new Error(`Invalid timing for ${name}: ${milliseconds}`);
  }
  const values = timings.get(name) ?? [];
  values.push(milliseconds);
  timings.set(name, values);
}

export function clearTimings(): void {
  timings.clear();
}

function percentile(values: number[], percentage: number): number {
  const sorted = [...values].sort((a, b) => a - b);
  const rank = Math.max(0, Math.ceil((percentage / 100) * sorted.length) - 1);
  return sorted[rank] ?? 0;
}

export type TimingReport = Record<string, { count: number; p50: number; p95: number; budget?: number }>;

function configuredBudget(name: string): number | undefined {
  const budgetName = budgetAliases[name] ?? name;
  const variable = budgetName === "bootstrap"
    ? "TAURI_E2E_P95_BOOTSTRAP_MS"
    : budgetName === "ipc"
      ? "TAURI_E2E_P95_IPC_MS"
      : undefined;
  if (variable) {
    const value = Number(process.env[variable]);
    if (Number.isFinite(value) && value >= 0) return value;
  }
  return DEFAULT_TIMING_BUDGETS[budgetName];
}

export function timingReport(outputRoot?: string): TimingReport {
  const report: TimingReport = Object.fromEntries(
    [...timings.entries()].map(([name, values]) => [name, {
      count: values.length,
      p50: percentile(values, 50),
      p95: percentile(values, 95),
      ...(configuredBudget(name) === undefined ? {} : { budget: configuredBudget(name) }),
    }]),
  );
  if (outputRoot) {
    mkdirSync(outputRoot, { recursive: true });
    writeFileSync(join(outputRoot, "timings.json"), `${JSON.stringify(report, null, 2)}\n`);
  }
  return report;
}

export function assertTimingBudget(name: string, p95LimitMilliseconds = configuredBudget(name) ?? Number.POSITIVE_INFINITY): void {
  if (!Number.isFinite(p95LimitMilliseconds) || p95LimitMilliseconds < 0) {
    throw new Error(`Invalid p95 budget for ${name}: ${p95LimitMilliseconds}`);
  }
  const report = timingReport()[name];
  if (!report) throw new Error(`No timing samples recorded for ${name}`);
  if (report.p95 > p95LimitMilliseconds) {
    throw new Error(`Timing budget exceeded for ${name}: p95=${report.p95}ms > ${p95LimitMilliseconds}ms`);
  }
}

/** Enforce only configured budgets; observational timings never block a run. */
export function assertConfiguredTimingBudgets(): void {
  for (const name of timings.keys()) {
    if (configuredBudget(name) !== undefined) assertTimingBudget(name);
  }
}

async function callOptional(browser: BrowserLike, method: string, ...args: unknown[]): Promise<unknown> {
  const candidate = browser[method];
  if (typeof candidate !== "function") return undefined;
  try {
    return await (candidate as (...values: unknown[]) => unknown)(...args);
  } catch (error) {
    return { error: error instanceof Error ? error.message : String(error) };
  }
}

function normalizeSpecName(value: string): string {
  const pathName = basename(value.replaceAll("\\", "/"));
  return pathName === "" ? sanitizeArtifactName(value) : pathName;
}

function sanitizeManifestPath(value: string | undefined): string | undefined {
  if (!value) return value;
  return value
    .split(/[\\/]+/u)
    .filter(Boolean)
    .map(sanitizeArtifactName)
    .join("/");
}

export async function writeE2eManifest(outputRoot: string, manifest: E2eManifest): Promise<string> {
  await mkdir(outputRoot, { recursive: true });
  const normalized: E2eManifest = {
    ...manifest,
    schemaVersion: manifest.schemaVersion ?? 1,
    results: [...manifest.results]
      .map((result) => ({
        ...result,
        spec: normalizeSpecName(result.spec),
        screenshot: sanitizeManifestPath(result.screenshot),
        snapshot: sanitizeManifestPath(result.snapshot),
        frontendLog: sanitizeManifestPath(result.frontendLog),
        backendLog: sanitizeManifestPath(result.backendLog),
      }))
      .sort((left, right) => left.spec.localeCompare(right.spec)),
  };
  const manifestPath = join(outputRoot, "manifest.json");
  await writeFile(manifestPath, `${JSON.stringify(normalized, null, 2)}\n`);
  return manifestPath;
}

async function hasNonEmptyFile(path: string): Promise<boolean> {
  try {
    const contents = await readFile(path);
    return contents.length > 0;
  } catch {
    return false;
  }
}

export async function assertE2eEvidence(
  outputRoot: string,
  expectedSpecs: string[],
  expectedRunId?: string,
): Promise<void> {
  const junitRoot = join(outputRoot, "junit");
  let junitFiles: string[] = [];
  try {
    junitFiles = (await readdir(junitRoot, { withFileTypes: true }))
      .filter((entry) => entry.isFile() && entry.name.toLowerCase().endsWith(".xml"))
      .map((entry) => join(junitRoot, entry.name));
  } catch {
    // The category-specific error below is intentionally stable for wrappers.
  }
  if (!(await Promise.all(junitFiles.map(hasNonEmptyFile))).some(Boolean)) {
    throw new Error(`Missing required JUnit artifact under ${junitRoot}`);
  }

  const manifestPath = join(outputRoot, "manifest.json");
  let manifest: E2eManifest;
  try {
    manifest = JSON.parse(await readFile(manifestPath, "utf8")) as E2eManifest;
  } catch {
    throw new Error(`Missing required manifest artifact: ${manifestPath}`);
  }
  if (!Array.isArray(manifest.results)) {
    throw new Error(`Manifest has no result entries: ${manifestPath}`);
  }
  if (expectedRunId && manifest.runId !== expectedRunId) {
    throw new Error(`Manifest run ID is stale: expected ${expectedRunId}, received ${manifest.runId ?? "missing"}`);
  }

  for (const expected of expectedSpecs) {
    const normalizedExpected = normalizeSpecName(expected);
    const expectedSurface = normalizedExpected.replace(/\.e2e\.ts$/iu, "");
    const expectedSpec = normalizedExpected.endsWith(".e2e.ts");
    const result = manifest.results.find((entry) =>
      entry.surface &&
      normalizeSpecName(entry.surface) === expectedSurface &&
      (!expectedSpec || normalizeSpecName(entry.spec) === normalizedExpected),
    );
    if (!result) {
      throw new Error(`Manifest is missing result for expected spec: ${normalizedExpected}`);
    }
    if (result?.status !== "passed") {
      throw new Error(`Manifest result is not passing for expected spec: ${normalizedExpected}`);
    }
    const junitPath = join(junitRoot, sanitizeArtifactName(normalizeSpecName(result.spec)) + ".xml");
    if (!(await hasNonEmptyFile(junitPath))) {
      throw new Error(`Missing required JUnit artifact for expected spec: ${normalizedExpected}`);
    }
  }

  const timingsPath = join(outputRoot, "timings.json");
  if (!(await hasNonEmptyFile(timingsPath))) {
    throw new Error(`Missing required timings artifact: ${timingsPath}`);
  }
  try {
    const timingData = JSON.parse(await readFile(timingsPath, "utf8")) as unknown;
    if (!timingData || typeof timingData !== "object" || Array.isArray(timingData)) {
      throw new Error("timings must be an object");
    }
  } catch {
    throw new Error(`Invalid required timings artifact: ${timingsPath}`);
  }
}

export async function saveE2eArtifacts(browser: BrowserLike, outputDir: string, name: string): Promise<void> {
  const artifactDir = join(outputDir, sanitizeArtifactName(name));
  await mkdir(artifactDir, { recursive: true });

  await callOptional(browser, "saveScreenshot", join(artifactDir, "screenshot.png"));
  const snapshot = await callOptional(browser, "execute", () => {
    const visit = (element: Element): Record<string, unknown> => ({
      role: element.getAttribute("role") ?? element.tagName.toLowerCase(),
      name: element.getAttribute("aria-label") ?? element.textContent?.trim().slice(0, 160) ?? "",
      children: [...element.children].map(visit),
    });
    return visit(document.body);
  });
  const logs = await callOptional(browser, "getLogs", "browser");
  const url = await callOptional(browser, "getUrl");
  const title = await callOptional(browser, "getTitle");

  await writeFile(join(artifactDir, "accessibility-snapshot.json"), JSON.stringify({ snapshot }, null, 2));
  await writeFile(join(artifactDir, "browser-state.json"), JSON.stringify({ url, title, logs }, null, 2));
  await writeFile(join(artifactDir, "timings.json"), JSON.stringify(timingReport(), null, 2));
}
