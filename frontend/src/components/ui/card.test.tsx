import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Card, CardContent, CardSm } from "./card";
import { Badge } from "./badge";

describe("Rust/UI Primitives", () => {
  it("renders CardSm with high-density padding and no nested blur", () => {
    render(
      <CardSm data-testid="dense-card">
        <CardContent>Content</CardContent>
      </CardSm>
    );
    const el = screen.getByTestId("dense-card");
    expect(el.className).toContain("p-3");
    expect(el.className).toContain("bg-card/60");
    expect(el.className).not.toContain("backdrop-blur");
  });

  it("renders standard Card without nested backdrop blur (anti-blur rule)", () => {
    render(
      <Card data-testid="standard-card">
        <CardContent>Content</CardContent>
      </Card>
    );
    const el = screen.getByTestId("standard-card");
    expect(el.className).not.toContain("backdrop-blur");
  });

  it("renders Badge with pill variant and active scale feedback", () => {
    render(<Badge variant="pill">Active</Badge>);
    const badge = screen.getByText("Active");
    expect(badge.className).toContain("rounded-full");
    expect(badge.className).toContain("active:scale-[0.97]");
  });
});
