import { useEffect, useMemo, useState } from "react"
import { PageHeader } from "../components/page-header"
import { Button } from "../components/ui/button"
import { Input } from "../components/ui/input"
import { chooseFolder, errorText, invoke, isTauri, onBob } from "../lib/api"
import { demoBiggest, demoChildren, demoRoot } from "../lib/demo"
import { bytes } from "../lib/format"
import { kindColor, treemap } from "../lib/treemap"
import type { Entry } from "../lib/types"
import { ask, toast } from "../store"

type Progress = { files: number; bytes: number; path: string } | null

export function DiskPage() {
  const [roots, setRoots] = useState<string[]>(isTauri() ? [] : ["C:\\", "D:\\"])
  const [path, setPath] = useState(isTauri() ? "" : demoRoot.path)
  const [stack, setStack] = useState<string[]>([])
  const [entries, setEntries] = useState<Entry[]>(isTauri() ? [] : demoChildren(demoRoot.path))
  const [biggest, setBiggest] = useState<Entry[]>(isTauri() ? [] : demoBiggest(false))
  const [query, setQuery] = useState("")
  const [hits, setHits] = useState<Entry[]>([])
  const [busy, setBusy] = useState(false)
  const [progress, setProgress] = useState<Progress>(null)
  const [error, setError] = useState("")
  const [mode, setMode] = useState<"arquivos" | "pastas">("arquivos")

  useEffect(() => {
    if (!isTauri()) return
    void invoke<string[]>("list_roots").then(setRoots).catch((err) => setError(errorText(err)))
    return () => {
      void invoke("cancel_scan")
    }
  }, [])

  useEffect(() => {
    if (!isTauri()) return
    let stop = () => {}
    void onBob((event) => {
      if (event.type === "scanProgress") setProgress({ files: event.files, bytes: event.bytes, path: event.path })
    }).then((unlisten) => {
      stop = unlisten
    })
    return () => stop()
  }, [])

  async function openPath(next: string, push = true) {
    setError("")
    setHits([])
    setQuery("")
    if (!isTauri()) {
      if (push && path) setStack((current) => [...current, path])
      setPath(next)
      setEntries(demoChildren(next))
      return
    }
    setBusy(true)
    setProgress({ files: 0, bytes: 0, path: next })
    try {
      if (push && path) setStack((current) => [...current, path])
      setPath(next)
      await invoke("start_scan", { path: next })
      const [children, files] = await Promise.all([
        invoke<Entry[]>("scan_children", { path: next }),
        invoke<Entry[]>("scan_biggest", { folders: false, query: "" }),
      ])
      setEntries(children)
      setBiggest(files)
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusy(false)
      setProgress(null)
    }
  }

  async function drill(entry: Entry) {
    if (!entry.dir || entry.synthetic) return
    if (!isTauri()) {
      setStack((current) => [...current, path])
      setPath(entry.path)
      setEntries(demoChildren(entry.path))
      return
    }
    const children = await invoke<Entry[]>("scan_children", { path: entry.path })
    setStack((current) => [...current, path])
    setPath(entry.path)
    setEntries(children)
  }

  async function back() {
    const previous = stack.at(-1)
    if (!previous) return
    setStack((current) => current.slice(0, -1))
    if (!isTauri()) {
      setPath(previous)
      setEntries(demoChildren(previous))
      return
    }
    const children = await invoke<Entry[]>("scan_children", { path: previous })
    setPath(previous)
    setEntries(children)
  }

  async function search() {
    if (!query.trim()) return
    if (!isTauri()) {
      setHits(demoBiggest(false).filter((entry) => entry.name.toLowerCase().includes(query.toLowerCase())))
      return
    }
    setHits(await invoke<Entry[]>("scan_biggest", { folders: false, query }))
  }

  async function showBiggest(next: "arquivos" | "pastas") {
    setMode(next)
    if (!isTauri()) {
      setBiggest(demoBiggest(next === "pastas"))
      return
    }
    setBiggest(await invoke<Entry[]>("scan_biggest", { folders: next === "pastas", query: "" }))
  }

  function remove(entry: Entry) {
    if (entry.synthetic) return
    ask("mandar para a lixeira", entry.path, "excluir", () => {
      void (async () => {
        if (!isTauri()) {
          toast("no navegador nada é apagado")
          return
        }
        const report = await invoke<{ deleted: string[]; failed: { path: string; error: string }[] }>("delete_to_trash", { paths: [entry.path] })
        if (report.failed.length) toast(report.failed[0].error)
        else toast("foi para a lixeira")
        setEntries((current) => current.filter((item) => item.path !== entry.path))
      })()
    })
  }

  async function reveal(entry: Entry) {
    if (!isTauri()) {
      toast("no navegador o explorador não abre")
      return
    }
    await invoke("open_in_explorer", { path: entry.path })
  }

  const boxes = useMemo(() => treemap(entries, 1000, 420), [entries])

  return (
    <div>
      <PageHeader title="mapa do disco" sub="vê o que ocupa o disco. pastas grandes viram blocos. o que você excluir daqui vai para a lixeira." />
      <div className="mb-4 flex flex-wrap items-center gap-2">
        {roots.map((root) => (
          <Button key={root} variant={path === root ? "default" : "outline"} size="sm" onClick={() => void openPath(root, false)}>
            {root}
          </Button>
        ))}
        <Button
          variant="ghost"
          size="sm"
          onClick={() => {
            void chooseFolder().then((picked) => {
              if (picked) void openPath(picked, false)
              else if (!isTauri()) toast("no navegador a pasta não abre")
            })
          }}
        >
          escolher pasta
        </Button>
        {stack.length > 0 && (
          <Button variant="ghost" size="sm" onClick={() => void back()}>
            voltar
          </Button>
        )}
      </div>
      {path && <p className="mb-3 truncate text-[13px] text-muted">{path}</p>}
      {busy && (
        <p className="mb-3 text-[13px] text-muted">
          lendo {progress ? `${new Intl.NumberFormat("pt-BR").format(progress.files)} arquivos · ${bytes(progress.bytes)}` : "…"}
        </p>
      )}
      {error && <p className="mb-3 text-[13px] text-danger">{error}</p>}
      {!path && !busy && <p className="text-[14px] text-muted">escolha um disco ou uma pasta para começar.</p>}
      {path && entries.length === 0 && !busy && <p className="text-[14px] text-muted">esta pasta está vazia.</p>}
      {entries.length > 0 && (
        <div className="overflow-hidden rounded-2xl border border-line bg-surface">
          <svg viewBox="0 0 1000 420" preserveAspectRatio="none" className="h-[360px] w-full">
            {boxes.map((box) => (
              <g key={box.path}>
                <rect
                  x={box.x + 1}
                  y={box.y + 1}
                  width={Math.max(0, box.w - 2)}
                  height={Math.max(0, box.h - 2)}
                  rx="6"
                  fill={kindColor(box.kind)}
                  className={box.dir ? "cursor-pointer" : undefined}
                  onClick={() => void drill(box)}
                />
                {box.w > 90 && box.h > 36 && (
                  <text x={box.x + 10} y={box.y + 22} fill="#1b1b19" fontSize="13">
                    {box.name.slice(0, 22)}
                  </text>
                )}
                {box.w > 90 && box.h > 52 && (
                  <text x={box.x + 10} y={box.y + 40} fill="#1b1b19" opacity="0.7" fontSize="12">
                    {bytes(box.size)}
                  </text>
                )}
              </g>
            ))}
          </svg>
        </div>
      )}
      {entries.length > 0 && (
        <div className="mt-6 grid gap-6 lg:grid-cols-[1.2fr_0.8fr]">
          <div className="overflow-hidden rounded-2xl border border-line">
            {entries.map((entry) => (
              <div key={entry.path} className="flex items-center gap-3 border-b border-line px-4 py-3 last:border-b-0">
                <span className="h-2.5 w-2.5 shrink-0 rounded-full" style={{ background: kindColor(entry.kind) }} />
                <button type="button" className="min-w-0 flex-1 truncate text-left text-[14px]" onClick={() => void drill(entry)}>
                  {entry.name}
                </button>
                <span className="text-[13px] text-muted">{bytes(entry.size)}</span>
                <button type="button" className="text-[13px] text-muted hover:text-ink" onClick={() => void reveal(entry)}>
                  abrir
                </button>
                <button type="button" className="text-[13px] text-danger" disabled={entry.synthetic} onClick={() => remove(entry)}>
                  lixeira
                </button>
              </div>
            ))}
          </div>
          <div>
            <div className="mb-3 flex gap-2">
              <Button size="sm" variant={mode === "arquivos" ? "default" : "outline"} onClick={() => void showBiggest("arquivos")}>
                maiores arquivos
              </Button>
              <Button size="sm" variant={mode === "pastas" ? "default" : "outline"} onClick={() => void showBiggest("pastas")}>
                maiores pastas
              </Button>
            </div>
            <div className="mb-3 flex gap-2">
              <Input value={query} placeholder="buscar nesta leitura" onChange={(event) => setQuery(event.target.value)} onKeyDown={(event) => event.key === "Enter" && void search()} />
              <Button variant="outline" onClick={() => void search()}>
                buscar
              </Button>
            </div>
            <div className="flex flex-col gap-2">
              {(hits.length ? hits : biggest).map((entry) => (
                <button key={entry.path} type="button" className="flex items-center justify-between gap-3 rounded-xl bg-soft px-3 py-2 text-left" onClick={() => void reveal(entry)}>
                  <span className="min-w-0 truncate text-[13px]">{entry.name}</span>
                  <span className="shrink-0 text-[12px] text-muted">{bytes(entry.size)}</span>
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
