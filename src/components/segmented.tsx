import { cn } from "../lib/cn"

export function Segmented<T extends string>({
  value,
  onChange,
  options,
}: {
  value: T
  onChange: (value: T) => void
  options: { value: T; label: string }[]
}) {
  return (
    <div className="flex rounded-full bg-soft p-1">
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          onClick={() => onChange(option.value)}
          className={cn(
            "h-8 whitespace-nowrap rounded-full px-3 text-[13px] transition-colors",
            value === option.value ? "bg-surface text-ink" : "text-muted hover:text-ink",
          )}
        >
          {option.label}
        </button>
      ))}
    </div>
  )
}
