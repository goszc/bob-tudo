import * as CheckboxPrimitive from "@radix-ui/react-checkbox"

export function Checkbox({
  checked,
  onCheckedChange,
  label,
  disabled,
}: {
  checked: boolean
  onCheckedChange: (value: boolean) => void
  label: string
  disabled?: boolean
}) {
  return (
    <CheckboxPrimitive.Root
      checked={checked}
      disabled={disabled}
      onCheckedChange={(value) => onCheckedChange(value === true)}
      aria-label={label}
      className="flex h-5 w-5 shrink-0 items-center justify-center rounded-[6px] border border-line bg-surface outline-none data-[state=checked]:border-fill data-[state=checked]:bg-fill disabled:opacity-40"
    >
      <CheckboxPrimitive.Indicator>
        <svg viewBox="0 0 12 12" className="h-3 w-3 text-onfill" aria-hidden="true">
          <path d="M2 6.2 L4.6 8.6 L10 3.2" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
        </svg>
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  )
}
