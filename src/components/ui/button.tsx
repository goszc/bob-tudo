import { Slot } from "@radix-ui/react-slot"
import { cva, type VariantProps } from "class-variance-authority"
import { forwardRef, type ButtonHTMLAttributes } from "react"
import { cn } from "../../lib/cn"

const buttonVariants = cva(
  "inline-flex cursor-pointer items-center justify-center gap-2 whitespace-nowrap rounded-[10px] text-[14px] font-medium transition-[transform,background-color,opacity,color] duration-200 ease-out disabled:pointer-events-none disabled:opacity-40 outline-none focus-visible:ring-2 focus-visible:ring-ink/25 active:scale-[0.98]",
  {
    variants: {
      variant: {
        default: "bg-fill text-onfill hover:opacity-90",
        ghost: "bg-transparent text-ink hover:bg-soft",
        outline: "border border-line bg-surface text-ink hover:bg-soft",
        quiet: "bg-transparent text-muted hover:text-ink",
      },
      size: {
        default: "h-10 px-4",
        sm: "h-8 px-3 text-[13px] rounded-[8px]",
        lg: "h-11 px-5",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
)

export interface ButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { className, variant, size, asChild = false, type = "button", ...props },
  ref,
) {
  const Comp = asChild ? Slot : "button"
  return <Comp ref={ref} type={type} className={cn(buttonVariants({ variant, size }), className)} {...props} />
})
