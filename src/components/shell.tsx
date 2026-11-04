import { useEffect, useMemo, useState, type ReactNode } from "react"
import { cn } from "../lib/cn"
import { isPage, type Page } from "../lib/types"
import { applyUpdate, closeConfirm, go, useApp } from "../store"
import { paletteActions } from "../lib/desk-demo"
import { Logo } from "./logo"
import { ConfirmDialog } from "./ui/dialog"
import { Input } from "./ui/input"

const GROUPS: { label: string; items: { id: Page; label: string }[] }[] = [
  { label: "agora", items: [{ id: "home", label: "sugestões" }] },
  {
    label: "disco",
    items: [
      { id: "disk", label: "mapa" },
      { id: "search", label: "busca" },
      { id: "history", label: "histórico" },
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
      { id: "network", label: "rede" },
      { id: "power", label: "energia" },
      { id: "game", label: "jogo" },
      { id: "drivers", label: "drivers" },
      { id: "privacy", label: "privacidade" },
      { id: "restore", label: "restauração" },
    ],
  },
  {
    label: "apps",
    items: [
      { id: "catalog", label: "catálogo" },
      { id: "backup", label: "backup" },
      { id: "upgrades", label: "atualizar" },
      { id: "uninstall", label: "desinstalar" },
    ],
  },
  { label: "app", items: [{ id: "report", label: "relatório" }, { id: "settings", label: "ajustes" }] },
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
        <TaskList />
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
        <TaskStrip />
        <main className="relative flex-1 overflow-hidden">{children}</main>
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
      <CommandPalette />
    </div>
  )
}

function CommandPalette() {
  const [open, setOpen] = useState(false)
  const [text, setText] = useState("")
  const actions = useMemo(() => {
    const needle = text.trim().toLowerCase()
    return paletteActions.filter((action) => !needle || action.label.includes(needle) || action.hint.includes(needle))
  }, [text])

  useEffect(() => {
    if (new URLSearchParams(location.search).get("palette") === "1") setOpen(true)
    const onKey = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault()
        setOpen((current) => !current)
      } else if (event.key === "Escape") {
        setOpen(false)
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [])

  if (!open) return null
  return (
    <div className="fixed inset-0 z-40 flex items-start justify-center bg-ink/20 px-4 pt-[12vh]" onClick={() => setOpen(false)}>
      <div className="w-full max-w-[520px] overflow-hidden rounded-2xl border border-line bg-surface shadow-xl" onClick={(event) => event.stopPropagation()}>
        <div className="border-b border-line p-3">
          <Input autoFocus value={text} onChange={(event) => setText(event.target.value)} placeholder="buscar tela, ação ou app" />
        </div>
        <div className="max-h-[360px] overflow-auto p-2">
          {actions.map((action) => (
            <button
              key={action.id}
              type="button"
              className="flex w-full items-center justify-between rounded-[10px] px-3 py-2 text-left hover:bg-soft"
              onClick={() => {
                if (isPage(action.page)) go(action.page)
                setOpen(false)
              }}
            >
              <span className="text-[14px]">{action.label}</span>
              <span className="text-[12px] text-muted">{action.hint}</span>
            </button>
          ))}
        </div>
      </div>
    </div>
  )
}

function TaskList() {
  const jobs = useApp().jobs.filter((job) => job.state === "running")
  if (!jobs.length) return null
  return (
    <div className="mt-4 border-t border-line pt-3">
      <div className="px-3 pb-1 text-[11px] text-muted">em andamento</div>
      <div className="flex flex-col gap-1">
        {jobs.map((job) => (
          <button key={job.id} type="button" onClick={() => isPage(job.page) && go(job.page)} className="rounded-[10px] px-3 py-2 text-left hover:bg-soft">
            <div className="flex items-center justify-between gap-2 text-[13px]">
              <span className="truncate">{job.title}</span>
              <span className="text-[11px] text-muted">{job.progress < 0 ? "…" : `${Math.round(job.progress * 100)}%`}</span>
            </div>
            <div className="mt-1 h-[3px] overflow-hidden rounded-full bg-soft">
              <div className="h-full rounded-full bg-ink" style={{ width: job.progress < 0 ? "40%" : `${Math.max(8, Math.min(100, job.progress * 100))}%` }} />
            </div>
          </button>
        ))}
      </div>
    </div>
  )
}

function TaskStrip() {
  const jobs = useApp().jobs.filter((job) => job.state === "running")
  if (!jobs.length) return null
  return (
    <div className="flex gap-2 overflow-x-auto border-b border-line px-4 py-2 md:hidden">
      {jobs.map((job) => (
        <button key={job.id} type="button" onClick={() => isPage(job.page) && go(job.page)} className="shrink-0 rounded-full bg-soft px-3 py-1 text-[12px]">
          {job.title} {job.progress < 0 ? "" : `${Math.round(job.progress * 100)}%`}
        </button>
      ))}
    </div>
  )
}
