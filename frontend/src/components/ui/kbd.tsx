import * as React from "react";

import { cn } from "../../lib/utils";

/** Minimal shadcn-style keyboard badge (plain element, no Radix needed). */
const Kbd = React.forwardRef<
  HTMLSpanElement,
  React.HTMLAttributes<HTMLSpanElement>
>(({ className, ...props }, ref) => (
  <kbd
    ref={ref}
    className={cn(
      "pointer-events-none inline-flex h-5 select-none items-center gap-1 rounded border border-foreground/20 bg-foreground/10 px-1.5 font-mono text-[11px] font-medium text-foreground/80",
      className,
    )}
    {...props}
  />
));
Kbd.displayName = "Kbd";

export { Kbd };
