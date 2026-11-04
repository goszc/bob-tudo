export function Progress({ value, active }: { value: number; active?: boolean }) {
  const known = value > 0 && value < 1
  return (
    <div className="h-[3px] overflow-hidden rounded-full bg-soft">
      <div
        className={known || !active ? "h-full rounded-full bg-ink transition-[width] duration-300 ease-out" : "bar-indeterminate h-full rounded-full bg-ink"}
        style={known || !active ? { width: `${Math.max(0, Math.min(1, value)) * 100}%` } : undefined}
      />
    </div>
  )
}
