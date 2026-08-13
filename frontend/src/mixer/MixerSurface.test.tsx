import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { MixerSurface } from "./MixerSurface";

describe("MixerSurface placeholder", () => {
  it("renders the placeholder text", () => {
    render(<MixerSurface />);
    expect(screen.getByText("Mixer placeholder")).toBeInTheDocument();
  });
});
