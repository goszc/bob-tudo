import { forwardRef, type InputHTMLAttributes } from "react"
import { cn } from "../../lib/cn"

export const Input = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(function Input(
  { className, ...props },
  ref,
) {
  return (
    <input
      ref={ref}
      className={cn(
        "h-10 w-full rounded-[10px] border border-line bg-surface px-3 text-[14px] outline-none transition-colors placeholder:text-muted focus:border-ink/40",
        className,
      )}
      {...props}
    />
  )
})
