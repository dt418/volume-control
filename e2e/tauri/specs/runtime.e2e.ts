import { browser, expect } from "@wdio/globals";

describe("Tauri runtime bridge", () => {
  it("exposes the global API and WDIO guest bridge", async () => {
    const wdioBrowser = browser as unknown as {
      tauri?: { switchWindow?: (label: string) => Promise<void> };
      getWindowHandles?: () => Promise<string[]>;
      getTitle?: () => Promise<string>;
      getUrl?: () => Promise<string>;
    };
    await wdioBrowser.tauri?.switchWindow?.("window-mixer");
    console.log("WebDriver context", {
      handles: await wdioBrowser.getWindowHandles?.(),
      title: await wdioBrowser.getTitle?.(),
      url: await wdioBrowser.getUrl?.(),
    });
    const state = await (browser as unknown as {
      execute: (script: () => unknown) => Promise<{
        globalTauri: string;
        internals: string;
        invoke: string;
        wdio: string;
        wdioExecute: string;
        scripts: string[];
        wdioScript: boolean;
        errors: string[];
      }>;
    }).execute(() => {
      const win = window as Window & {
        __TAURI__?: { core?: { invoke?: unknown } };
        __TAURI_INTERNALS__?: unknown;
        wdioTauri?: { execute?: unknown };
        __volumecontrol_e2e_errors?: string[];
      };
      return {
        globalTauri: typeof win.__TAURI__,
        internals: typeof win.__TAURI_INTERNALS__,
        invoke: typeof win.__TAURI__?.core?.invoke,
        wdio: typeof win.wdioTauri,
        wdioExecute: typeof win.wdioTauri?.execute,
        scripts: Array.from(document.scripts, (script) => script.src || script.textContent?.slice(0, 40) || "inline"),
        wdioScript: Boolean(document.querySelector('script[src*="tauri-plugin.wdio.js"]')),
        errors: win.__volumecontrol_e2e_errors ?? [],
      };
    });
    console.log("Tauri runtime bridge state", state);
    expect(state.globalTauri).toBe("object");
    expect(state.invoke).toBe("function");
    expect(state.wdio).toBe("object");
    expect(state.wdioExecute).toBe("function");
  });
});
