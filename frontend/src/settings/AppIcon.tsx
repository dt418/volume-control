import { cn } from "../lib/utils";

/** Shell icon with a letter-tile fallback when extraction is unavailable. */
export function AppIcon({
  src,
  label,
  className,
}: {
  src?: string;
  label: string;
  className?: string;
}) {
  const box = cn("h-4 w-4 shrink-0 rounded-sm", className);
  if (src) {
    return <img src={src} alt="" aria-hidden="true" className={cn(box, "object-contain")} />;
  }
  return (
    <span
      aria-hidden="true"
      className={cn(
        box,
        "inline-flex items-center justify-center bg-foreground/10 text-[9px] font-semibold uppercase text-foreground/70",
      )}
    >
      {label.trim().charAt(0) || "?"}
    </span>
  );
}
