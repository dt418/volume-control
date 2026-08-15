import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";

import { ShortcutRecorder } from "./ShortcutRecorder";

describe("ShortcutRecorder", () => {
  it("records a modifier plus key in portable Code format", () => {
    const onChange = vi.fn();
    render(<ShortcutRecorder value="Ctrl+Alt+KeyV" onChange={onChange} />);

    fireEvent.click(screen.getByRole("button", { name: /Record/ }));
    fireEvent.keyDown(window, {
      key: "U",
      code: "KeyU",
      ctrlKey: true,
      shiftKey: true,
    });

    expect(onChange).toHaveBeenCalledWith("Ctrl+Shift+KeyU");
    expect(screen.getByRole("button", { name: "Global shortcut: Ctrl+Alt+V" })).toBeInTheDocument();
  });

  it("keeps recording when a bare key is pressed and explains the requirement", () => {
    const onChange = vi.fn();
    render(<ShortcutRecorder value="" onChange={onChange} />);

    fireEvent.click(screen.getByRole("button", { name: /Global shortcut: Not set/i }));
    fireEvent.keyDown(window, { key: "U", code: "KeyU" });

    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByText(/modifier such as Ctrl/i)).toBeInTheDocument();
    expect(screen.getByText("Press shortcut…")).toBeInTheDocument();
  });

  it("supports Escape to cancel and Backspace to clear", () => {
    const onChange = vi.fn();
    render(<ShortcutRecorder value="Ctrl+Alt+KeyV" onChange={onChange} />);

    fireEvent.click(screen.getByRole("button", { name: /Record/ }));
    fireEvent.keyDown(window, { key: "Escape", code: "Escape" });
    expect(screen.queryByText("Press shortcut…")).not.toBeInTheDocument();
    expect(onChange).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: /Record/ }));
    fireEvent.keyDown(window, { key: "Backspace", code: "Backspace" });
    expect(onChange).toHaveBeenCalledWith("");

    fireEvent.click(screen.getByRole("button", { name: /Clear/ }));
    expect(onChange).toHaveBeenLastCalledWith("");
  });
});
