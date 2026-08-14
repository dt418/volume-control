import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

type BrowserLike = Record<string, unknown>;
const timings = new Map<string, number[]>();

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

export function timingReport(): Record<string, { count: number; p50: number; p95: number }> {
  return Object.fromEntries(
    [...timings.entries()].map(([name, values]) => [name, {
      count: values.length,
      p50: percentile(values, 50),
      p95: percentile(values, 95),
    }]),
  );
}

async function callOptional(browser: BrowserLike, method: string, ...args: unknown[]): Promise<unknown> {
  const candidate = browser[method];
  return typeof candidate === "function" ? await (candidate as (...values: unknown[]) => unknown)(...args) : undefined;
}

export async function saveE2eArtifacts(browser: BrowserLike, outputDir: string, name: string): Promise<void> {
  const artifactDir = join(outputDir, sanitizeArtifactName(name));
  await mkdir(artifactDir, { recursive: true });

  await callOptional(browser, "saveScreenshot", join(artifactDir, "screenshot.png"));
  const snapshot = await callOptional(browser, "execute", () => document.documentElement.outerHTML);
  const logs = await callOptional(browser, "getLogs", "browser");
  const url = await callOptional(browser, "getUrl");
  const title = await callOptional(browser, "getTitle");

  await writeFile(join(artifactDir, "accessibility-snapshot.json"), JSON.stringify({ snapshot }, null, 2));
  await writeFile(join(artifactDir, "browser-state.json"), JSON.stringify({ url, title, logs }, null, 2));
  await writeFile(join(artifactDir, "timings.json"), JSON.stringify(timingReport(), null, 2));
}
