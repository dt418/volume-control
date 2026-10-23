import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const css = readFileSync(resolve(__dirname, "styles.css"), "utf-8");

describe("styles.css cross-platform webview guards (docs/webview-compat.md)", () => {
  it("defines .glass-surface with a readable translucent fallback", () => {
    // WebKitGTK software rendering (WEBKIT_DISABLE_COMPOSITING_MODE=1) may
    // no-op backdrop-filter, so the background must be readable without it.
    expect(css).toMatch(/\.glass-surface\s*{[^}]*background-color:[^}]*}/);
  });

  it("emits both the -webkit- prefix and unprefixed backdrop-filter inside @supports", () => {
    // WebKit < Safari 18 / WebKitGTK < 2.46 need the prefix.
    expect(css).toContain("-webkit-backdrop-filter: blur(20px) saturate(180%)");
    expect(css).toContain("backdrop-filter: blur(20px) saturate(180%)");
    expect(css).toContain("@supports ((-webkit-backdrop-filter: blur(20px)) or (backdrop-filter: blur(20px)))");
  });

  it("defines specular rim highlight and squircle curvature for Liquid Glass", () => {
    expect(css).toContain("inset 0 1px 0 0");
    expect(css).toContain("border-radius: 1.125rem");
  });

  it("preserves single-pass anti-blur constraint", () => {
    const unprefixed = css.match(/(?:^|[^-])backdrop-filter:\s*blur/g);
    expect(unprefixed).not.toBeNull();
    expect(unprefixed!.length).toBeLessThanOrEqual(2);
    const allMatches = css.match(/backdrop-filter:\s*blur/g);
    expect(allMatches!.length).toBeLessThanOrEqual(4);
  });
});
