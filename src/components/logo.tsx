import { cn } from "../lib/cn"

export function Logo({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 32 32" className={cn("shrink-0", className)} aria-hidden="true">
      <circle cx="18.2" cy="13.6" r="10" fill="#2f7bff" />
      <ellipse cx="14.3" cy="12.8" rx="2.05" ry="2.55" fill="#102033" />
      <ellipse cx="21.7" cy="12.8" rx="2.05" ry="2.55" fill="#102033" />
      <rect x="1" y="18" width="20.5" height="12.5" rx="4.2" fill="#2451b5" />
    </svg>
  )
}
