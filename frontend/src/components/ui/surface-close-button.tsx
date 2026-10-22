import { X } from "lucide-react";
import type { ComponentProps } from "react";

import { cn } from "../../lib/utils";

interface SurfaceCloseButtonProps
  extends Omit<ComponentProps<"button">, "aria-label" | "children"> {
  /** Accessible name and native tooltip for the control. */
  label: string;
}

/** Consistent close affordance for every webview surface: a 32px hit target,
 *  a visible focus ring, and an icon that does not rely on font glyphs. */
export function SurfaceCloseButton({
  label,
  className,
  ...props
}: SurfaceCloseButtonProps) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className={cn(
        "inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-foreground/65 transition-colors hover:bg-foreground/10 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent active:bg-foreground/15",
        className,
      )}
      {...props}
    >
      <X aria-hidden="true" className="h-4 w-4" />
    </button>
  );
}
