import { describe, expect, it } from "vitest";

import { conflictSentence } from "./ConflictCallout";

describe("conflictSentence (legacy explanation_atoms parity)", () => {
  it("single combo uses the singular tail", () => {
    expect(conflictSentence(["Ctrl+Alt+M"], "is used by another app.")).toBe(
      "Ctrl+Alt+M is used by another app.",
    );
  });

  it("two combos join with 'and' and the plural tail", () => {
    expect(conflictSentence(["Ctrl+Alt+M", "Ctrl+Alt+V"], "are used by another app.")).toBe(
      "Ctrl+Alt+M and Ctrl+Alt+V are used by another app.",
    );
  });

  it("three+ combos use ', ' connectors and 'and' before the last", () => {
    expect(
      conflictSentence(["Ctrl+Alt+M", "Ctrl+Alt+V", "Ctrl+Alt+R"], "are used by another app."),
    ).toBe("Ctrl+Alt+M, Ctrl+Alt+V and Ctrl+Alt+R are used by another app.");
  });

  it("empty combo list renders nothing", () => {
    expect(conflictSentence([], "is used by another app.")).toBe("");
  });
});
