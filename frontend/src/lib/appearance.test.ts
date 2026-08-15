import { describe, expect, it, beforeEach, vi } from "vitest";
import { readFileSync } from "node:fs";
import { applyAppearance, type AppearancePayload } from "./appearance";

// No Tauri runtime in jsdom; the native setTheme sync is fire-and-forget.
vi.mock("@tauri-apps/api/app", () => ({
  setTheme: vi.fn(async () => {}),
}));

// Read the token source directly (Vite's `?raw` is unreliable inside the
// Tailwind pipeline); vitest rewrites import.meta.url, so resolve from cwd
// (vitest runs with the frontend/ directory as cwd).
const stylesCss = readFileSync(process.cwd() + "/src/styles.css", "utf8");

/** Parse "hsl(240 10% 3.9%)" → { h, s, l } in 0..1 ranges. */
function parseHsl(value: string): { h: number; s: number; l: number } {
  const match = /hsl\(\s*([\d.]+)\s+([\d.]+)%\s+([\d.]+)%\s*\)/.exec(value);
  if (!match) throw new Error(`cannot parse hsl: ${value}`);
  return { h: Number(match[1]), s: Number(match[2]) / 100, l: Number(match[3]) / 100 };
}

function toRgb(c: { h: number; s: number; l: number }): [number, number, number] {
  const { h, s, l } = c;
  const k = (n: number) => (n + h / 30) % 12;
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
  return [f(0), f(8), f(4)].map((v) => Math.round(v * 255)) as [number, number, number];
}

function luminance(rgb: [number, number, number]): number {
  const c = rgb.map((v) => {
    const x = v / 255;
    return x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
}

function contrastRatio(a: string, b: string): number {
  const la = luminance(toRgb(parseHsl(a)));
  const lb = luminance(toRgb(parseHsl(b)));
  const [hi, lo] = la > lb ? [la, lb] : [lb, la];
  return (hi + 0.05) / (lo + 0.05);
}

/** Extract a token from the raw CSS: the value of --name inside the first
 *  block whose selector contains `selector`. */
function token(selector: string, name: string): string {
  const block = /:root\s*\{([^}]*)\}/.exec(stylesCss);
  const darkBlock = /\.dark\s*\{([^}]*)\}/.exec(stylesCss);
  const source = selector === ".dark" ? darkBlock : block;
  if (!source) throw new Error(`block ${selector} not found in styles.css`);
  const line = new RegExp(`${name}\\s*:\\s*([^;]+);`).exec(source[1]);
  if (!line) throw new Error(`token ${name} not found in ${selector}`);
  return line[1].trim();
}

describe("theme tokens (styles.css)", () => {
  it("light mode: background and foreground have WCAG contrast >= 4.5", () => {
    const ratio = contrastRatio(token(":root", "--foreground"), token(":root", "--background"));
    expect(ratio).toBeGreaterThanOrEqual(4.5);
  });

  it("dark mode: background and foreground have WCAG contrast >= 4.5", () => {
    const ratio = contrastRatio(token(".dark", "--foreground"), token(".dark", "--background"));
    expect(ratio).toBeGreaterThanOrEqual(4.5);
  });

  it("light and dark backgrounds differ (theme is actually switchable)", () => {
    expect(token(":root", "--background")).not.toBe(token(".dark", "--background"));
  });

  it("muted-foreground stays readable on both backgrounds", () => {
    const light = contrastRatio(token(":root", "--muted-foreground"), token(":root", "--background"));
    const dark = contrastRatio(token(".dark", "--muted-foreground"), token(".dark", "--background"));
    expect(light).toBeGreaterThanOrEqual(4.5);
    expect(dark).toBeGreaterThanOrEqual(4.5);
  });
});

describe("applyAppearance", () => {
  beforeEach(() => {
    document.documentElement.classList.remove("dark", "reduced-motion");
    delete document.documentElement.dataset.theme;
    localStorage.clear();
  });

  const dark: AppearancePayload = {
    theme_resolved: "dark",
    material: "Auto",
    motion: "Reduced",
    accent: "System",
  };

  it("applies dark theme + reduced-motion and caches in localStorage", () => {
    applyAppearance(dark);
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(document.documentElement.classList.contains("reduced-motion")).toBe(true);
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(localStorage.getItem("app-theme")).toBe("dark");
  });

  it("switches back to light and removes the dark class", () => {
    applyAppearance(dark);
    applyAppearance({ ...dark, theme_resolved: "light", motion: "Full" });
    expect(document.documentElement.classList.contains("dark")).toBe(false);
    expect(document.documentElement.classList.contains("reduced-motion")).toBe(false);
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(localStorage.getItem("app-theme")).toBe("light");
  });
});
