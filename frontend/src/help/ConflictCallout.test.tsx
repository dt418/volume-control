import { describe, expect, it } from "vitest";

import { translatorFor } from "../lib/i18n";
import { conflictSentence } from "./ConflictCallout";

const en = translatorFor("en");

describe("conflictSentence (legacy explanation_atoms parity)", () => {
  it("single combo uses the singular tail", () => {
    expect(conflictSentence(["Ctrl+Alt+M"], "help.calloutTailOne", en)).toBe(
      "Ctrl+Alt+M is used by another app.",
    );
  });

  it("two combos join with 'and' and the plural tail", () => {
    expect(
      conflictSentence(["Ctrl+Alt+M", "Ctrl+Alt+V"], "help.calloutTailMany", en),
    ).toBe("Ctrl+Alt+M and Ctrl+Alt+V are used by another app.");
  });

  it("three+ combos use ', ' connectors and 'and' before the last", () => {
    expect(
      conflictSentence(
        ["Ctrl+Alt+M", "Ctrl+Alt+V", "Ctrl+Alt+R"],
        "help.calloutTailMany",
        en,
      ),
    ).toBe("Ctrl+Alt+M, Ctrl+Alt+V and Ctrl+Alt+R are used by another app.");
  });

  it("empty combo list renders nothing", () => {
    expect(conflictSentence([], "help.calloutTailOne", en)).toBe("");
  });

  it("localizes the tail through the translator", () => {
    const vi = translatorFor("vi");
    expect(conflictSentence(["Ctrl+Alt+M"], "help.calloutTailOne", vi)).toBe(
      "Ctrl+Alt+M đang bị ứng dụng khác dùng.",
    );
  });
});
