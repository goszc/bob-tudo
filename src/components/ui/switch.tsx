import * as SwitchPrimitives from "@radix-ui/react-switch"

export function Switch({
  checked,
  onCheckedChange,
  disabled,
  label,
}: {
  checked: boolean
  onCheckedChange: (value: boolean) => void
  disabled?: boolean
  label: string
}) {
  return (
    <SwitchPrimitives.Root
      checked={checked}
      onCheckedChange={onCheckedChange}
      disabled={disabled}
      aria-label={label}
      className="inline-flex h-6 w-11 shrink-0 items-center rounded-full bg-line outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ink/25 data-[state=checked]:bg-fill disabled:opacity-40"
    >
      <SwitchPrimitives.Thumb className="pointer-events-none block h-5 w-5 translate-x-0.5 rounded-full bg-white shadow-sm transition-transform data-[state=checked]:translate-x-[22px] data-[state=checked]:bg-onfill" />
    </SwitchPrimitives.Root>
  )
}
