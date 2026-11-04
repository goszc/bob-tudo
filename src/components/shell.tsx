import type { ReactNode } from "react"
import { cn } from "../lib/cn"
import type { Page } from "../lib/types"
import { applyUpdate, closeConfirm, go, useApp } from "../store"
import { Logo } from "./logo"
import { ConfirmDialog } from "./ui/dialog"

const GROUPS: { label: string; items: { id: Page; label: string }[] }[] = [
  {
    label: "disco",
    items: [
      { id: "disk", label: "mapa" },
      { id: "clean", label: "limpeza" },
      { id: "dupes", label: "duplicados" },
      { id: "health", label: "discos" },
    ],
  },
  {
    label: "sistema",
    items: [
      { id: "startup", label: "inicialização" },
      { id: "monitor", label: "monitor" },
      { id: "privacy", label: "privacidade" },
      { id: "restore", label: "restauração" },
    ],
  },
  {
    label: "apps",
    items: [
      { id: "install", label: "instalar" },
      { id: "oss", label: "código aberto" },
      { id: "upgrades", label: "atualizar" },
      { id: "uninstall", label: "desinstalar" },
    ],
  },
  { label: "app", items: [{ id: "settings", label: "ajustes" }] },
]

export function Shell({ children }: { children: ReactNode }) {
  const app = useApp()
  return (
    <div className="flex h-full min-h-screen bg-bg text-ink">
      <aside className="hidden w-[232px] shrink-0 flex-col border-r border-line px-3 py-5 md:flex">
        <div className="mb-6 flex items-center gap-2.5 px-2">
          <Logo className="h-7 w-7" />
          <span className="text-[15px] font-medium tracking-[-0.03em]">bob tudo</span>
        </div>
        <nav className="flex flex-1 flex-col gap-4 overflow-auto">
          {GROUPS.map((group) => (
            <div key={group.label}>
              <div className="px-3 pb-1 text-[11px] text-muted">{group.label}</div>
              <div className="flex flex-col gap-0.5">
                {group.items.map((item) => (
                  <button
                    key={item.id}
                    type="button"
                    aria-current={app.page === item.id ? "page" : undefined}
                    onClick={() => go(item.id)}
                    className={cn(
                      "flex h-9 w-full items-center rounded-[10px] px-3 text-left text-[14px] transition-colors",
                      app.page === item.id ? "bg-soft text-ink" : "text-muted hover:bg-soft/80 hover:text-ink",
                    )}
                  >
                    {item.label}
                  </button>
                ))}
              </div>
            </div>
          ))}
        </nav>
      </aside>
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex items-center gap-2 border-b border-line px-4 py-3 md:hidden">
          <Logo className="h-6 w-6" />
          <span className="text-[14px] font-medium">bob tudo</span>
        </div>
        <nav className="flex gap-1 overflow-x-auto border-b border-line px-3 py-2 md:hidden">
          {GROUPS.flatMap((group) => group.items).map((item) => (
            <button
              key={item.id}
              type="button"
              onClick={() => go(item.id)}
              className={cn(
                "h-8 shrink-0 rounded-full px-3 text-[13px]",
                app.page === item.id ? "bg-fill text-onfill" : "text-muted",
              )}
            >
              {item.label}
            </button>
          ))}
        </nav>
        <main className="flex-1 overflow-auto">
          <div key={app.page} className="animate-rise mx-auto w-full max-w-[1080px] px-5 py-8 md:px-10">
            {children}
          </div>
        </main>
      </div>
      {app.updateOffer ? (
        <div className="fixed bottom-6 left-1/2 z-30 flex -translate-x-1/2 items-center gap-3 rounded-full bg-fill px-4 py-2 text-[13px] text-onfill">
          <span>nova versão {app.updateOffer}. reiniciar?</span>
          <button type="button" className="underline" onClick={() => void applyUpdate()}>
            reiniciar
          </button>
        </div>
      ) : app.toast ? (
        <div className="pointer-events-none fixed bottom-6 left-1/2 z-30 -translate-x-1/2 rounded-full bg-fill px-4 py-2 text-[13px] text-onfill">
          {app.toast}
        </div>
      ) : null}
      <ConfirmDialog
        open={app.confirm !== null}
        title={app.confirm?.title ?? ""}
        body={app.confirm?.body ?? ""}
        confirm={app.confirm?.confirm ?? "confirmar"}
        cancel={app.confirm?.cancel ?? "cancelar"}
        onClose={closeConfirm}
        onConfirm={() => {
          const run = app.confirm?.run
          closeConfirm()
          run?.()
        }}
      />
    </div>
  )
}
