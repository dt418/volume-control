import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "./ipc";
import { markSurfaceReady } from "./surface";

vi.mock("./ipc", () => ({
  invoke: vi.fn(),
}));

describe("surface readiness", () => {
  beforeEach(() => vi.clearAllMocks());

  it("notifies Rust that the current webview is ready", async () => {
    vi.mocked(ipc.invoke).mockResolvedValue(undefined);

    await markSurfaceReady();

    expect(ipc.invoke).toHaveBeenCalledTimes(1);
    expect(ipc.invoke).toHaveBeenCalledWith("surface_ready");
  });
});
