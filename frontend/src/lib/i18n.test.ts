import { describe, expect, it } from "vitest";
import { asLanguage, translatorFor } from "./i18n";

describe("i18n dictionaries", () => {
  it("translates every key in both languages", () => {
    const en = translatorFor("en");
    const vi = translatorFor("vi");
    const keys = [
      "settings.title",
      "settings.general",
      "mixer.title",
      "overlay.volume",
      "help.title",
    ] as const;
    for (const key of keys) {
      expect(en(key)).toBeTruthy();
      expect(vi(key)).toBeTruthy();
    }
  });

  it("flips language-sensitive labels", () => {
    expect(translatorFor("en")("settings.title")).toBe("Settings");
    expect(translatorFor("vi")("settings.title")).toBe("Cài đặt");
    expect(translatorFor("vi")("overlay.volume")).toBe("Âm lượng");
  });

  it("normalizes unknown payload values to English", () => {
    expect(asLanguage("vi")).toBe("vi");
    expect(asLanguage("fr")).toBe("en");
    expect(asLanguage(undefined)).toBe("en");
  });
});
