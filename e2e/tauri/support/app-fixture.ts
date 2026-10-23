import { mkdir, rm } from "node:fs/promises";
import { randomUUID } from "node:crypto";
import { isAbsolute, resolve } from "node:path";

export interface AppFixtureOptions {
  binary?: string;
  outputRoot?: string;
  configDir?: string;
  preserveConfig?: boolean;
}

export interface AppFixture {
  binary: string;
  configDir: string;
  outputDir: string;
  runId: string;
  cleanup: () => Promise<void>;
}

function defaultBinary(): string {
  const repositoryRoot = resolve(import.meta.dirname, "../../..");
  const name = process.platform === "win32" ? "VolumeControl.exe" : "VolumeControl";
  return resolve(repositoryRoot, "target", "debug", name);
}

export async function createAppFixture(options: AppFixtureOptions = {}): Promise<AppFixture> {
  const runId = `${Date.now()}-${randomUUID().slice(0, 8)}`;
  const repositoryRoot = resolve(import.meta.dirname, "../../..");
  const outputDir = resolve(options.outputRoot ?? resolve(repositoryRoot, "output", "tauri-e2e"), runId);
  const envConfigDir = process.env.TAURI_E2E_CONFIG_DIR;
  const preserveConfig = options.preserveConfig ?? process.env.TAURI_E2E_PRESERVE_CONFIG === "1";
  const configDir = resolve(options.configDir ?? envConfigDir ?? resolve(outputDir, "config"));
  const binary = isAbsolute(options.binary ?? "") ? options.binary! : resolve(repositoryRoot, options.binary ?? defaultBinary());

  await mkdir(configDir, { recursive: true });

  let cleaned = false;
  const cleanup = async (): Promise<void> => {
    if (cleaned || preserveConfig) return;
    cleaned = true;
    await rm(configDir, { recursive: true, force: true });
  };

  return { binary, configDir, outputDir, runId, cleanup };
}
