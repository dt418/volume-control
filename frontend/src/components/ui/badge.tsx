import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "../../lib/utils";

const badgeVariants = cva(
  "inline-flex items-center gap-1 rounded-md border px-2 py-0.5 text-xs font-medium transition-colors",
  {
    variants: {
      variant: {
        default: "border-transparent bg-accent/20 text-accent",
        success:
          "border-transparent bg-green-500/15 text-green-600 dark:bg-green-500/20 dark:text-green-400",
        destructive:
          "border-transparent bg-red-500/15 text-red-500 dark:bg-red-500/20 dark:text-red-400",
        outline: "border-foreground/20 text-foreground/80",
      },
    },
    defaultVariants: { variant: "default" },
  },
);

function Badge({
  className,
  variant,
  ...props
}: React.ComponentProps<"span"> & VariantProps<typeof badgeVariants>) {
  return (
    <span className={cn(badgeVariants({ variant }), className)} {...props} />
  );
}

export { Badge, badgeVariants };
