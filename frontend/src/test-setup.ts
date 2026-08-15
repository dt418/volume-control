import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// vitest runs without `globals: true`, so @testing-library/react's automatic
// cleanup never registers; without this, DOM accumulates across tests.
afterEach(() => cleanup());

// framer-motion's `layout` animations measure via ResizeObserver; jsdom does
// not ship one. A no-op stub keeps layout-animated components renderable in
// tests without affecting production code.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}

if (typeof globalThis.ResizeObserver === "undefined") {
  globalThis.ResizeObserver = ResizeObserverStub as unknown as typeof ResizeObserver;
}
