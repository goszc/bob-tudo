import * as Dialog from "@radix-ui/react-dialog"
import { Button } from "./button"

export function ConfirmDialog({
  open,
  title,
  body,
  confirm,
  cancel,
  onConfirm,
  onClose,
}: {
  open: boolean
  title: string
  body: string
  confirm: string
  cancel: string
  onConfirm: () => void
  onClose: () => void
}) {
  return (
    <Dialog.Root open={open} onOpenChange={(next) => !next && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-ink/25" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-[min(420px,calc(100%-2rem))] -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-line bg-surface p-6">
          <Dialog.Title className="text-[16px] font-medium tracking-tight">{title}</Dialog.Title>
          <Dialog.Description className="mt-2 text-[14px] leading-relaxed text-muted">{body}</Dialog.Description>
          <div className="mt-6 flex justify-end gap-2">
            <Button variant="ghost" onClick={onClose}>
              {cancel}
            </Button>
            <Button onClick={onConfirm}>{confirm}</Button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
