import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { bandForValue, SignalRail } from "./SignalRail";

const thresholds = { green_up_to: 40, blue_up_to: 75, orange_up_to: 100 };

describe("bandForValue", () => {
  it("maps 0% and muted values to the gray band", () => {
    expect(bandForValue(0, false, thresholds)).toBe("gray");
    expect(bandForValue(55, true, thresholds)).toBe("gray");
  });

  it("maps values up to green_up_to to the green band", () => {
    expect(bandForValue(20, false, thresholds)).toBe("green");
    expect(bandForValue(40, false, thresholds)).toBe("green");
  });

  it("maps values between the green and blue thresholds to the blue band", () => {
    expect(bandForValue(41, false, thresholds)).toBe("blue");
    expect(bandForValue(75, false, thresholds)).toBe("blue");
  });

  it("maps values above the blue threshold to the orange band", () => {
    expect(bandForValue(76, false, thresholds)).toBe("orange");
    expect(bandForValue(100, false, thresholds)).toBe("orange");
  });
});

describe("SignalRail", () => {
  it("picks the fill band class matching the value", () => {
    const { container, rerender } = render(
      <SignalRail value={20} muted={false} thresholds={thresholds} />,
    );
    expect(container.querySelector(".rail-fill.band-green")).not.toBeNull();
    rerender(<SignalRail value={60} muted={false} thresholds={thresholds} />);
    expect(container.querySelector(".rail-fill.band-blue")).not.toBeNull();
    rerender(<SignalRail value={90} muted={false} thresholds={thresholds} />);
    expect(container.querySelector(".rail-fill.band-orange")).not.toBeNull();
    rerender(<SignalRail value={60} muted={true} thresholds={thresholds} />);
    expect(container.querySelector(".rail-fill.band-gray")).not.toBeNull();
  });

  it("renders a circle thumb normally and a diamond marker plus Muted label when muted", () => {
    const { container, rerender } = render(
      <SignalRail value={55} muted={false} thresholds={thresholds} />,
    );
    expect(container.querySelector(".rail-thumb")).not.toBeNull();
    expect(container.querySelector(".rail-muted-diamond")).toBeNull();

    rerender(<SignalRail value={55} muted={true} thresholds={thresholds} />);
    expect(container.querySelector(".rail-muted-diamond")).not.toBeNull();
    expect(container.querySelector(".rail-thumb")).toBeNull();
    expect(screen.getByText("Muted")).toBeInTheDocument();
  });
});
