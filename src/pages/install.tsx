import { useState } from "react"
import { PageHeader } from "../components/page-header"
import { Progress } from "../components/progress"
import { Button } from "../components/ui/button"
import { Checkbox } from "../components/ui/checkbox"
import { Input } from "../components/ui/input"
import { errorText, invoke, isTauri, onBob } from "../lib/api"
import { demoSearch } from "../lib/demo"
import type { WingetRow } from "../lib/types"
import { toast } from "../store"

export function InstallPage() {
  const [query, setQuery] = useState(isTauri() ? "" : "vlc")
  const [rows, setRows] = useState<WingetRow[]>(isTauri() ? [] : demoSearch)
  const [picked, setPicked] = useState<string[]>([])
  const [busy, setBusy] = useState(false)
  const [log, setLog] = useState("")
  const [error, setError] = useState("")

  async function search() {
    if (!query.trim()) return
    if (!isTauri()) {
      setRows(demoSearch)
      return
    }
    setBusy(true)
    setError("")
    try {
      setRows(await invoke<WingetRow[]>("winget_search", { query }))
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusy(false)
    }
  }

  async function install(ids: string[]) {
    if (!ids.length) {
      toast("escolha um aplicativo")
      return
    }
    if (!isTauri()) {
      toast("no navegador o winget não instala")
      return
    }
    setLog("")
    const stop = await onBob((event) => {
      if (event.type === "jobLog") setLog((current) => `${current}${event.line}\n`)
    })
    try {
      await invoke("winget_install", { ids })
      toast("instalação iniciada")
    } catch (err) {
      setError(errorText(err))
    } finally {
      window.setTimeout(stop, 1200)
    }
  }

  return (
    <div>
      <PageHeader title="instalar" sub="busca no winget e instala um ou vários de uma vez. o progresso aparece aqui embaixo." />
      <div className="mb-4 flex gap-2">
        <Input value={query} placeholder="nome ou id" onChange={(event) => setQuery(event.target.value)} onKeyDown={(event) => event.key === "Enter" && void search()} />
        <Button onClick={() => void search()} disabled={busy}>
          buscar
        </Button>
        <Button variant="outline" onClick={() => void install(picked)}>
          instalar selecionados
        </Button>
      </div>
      {busy && <Progress value={0} active />}
      {error && <p className="mt-3 text-[13px] text-danger">{error}</p>}
      {!rows.length && !busy && <p className="mt-4 text-[14px] text-muted">busque um aplicativo. o winget precisa estar no windows.</p>}
      <div className="mt-4 flex flex-col gap-2">
        {rows.map((row) => (
          <div key={row.id} className="flex items-center gap-3 rounded-2xl border border-line px-4 py-3">
            <Checkbox
              checked={picked.includes(row.id)}
              label={row.name}
              onCheckedChange={(checked) => setPicked((current) => (checked ? [...current, row.id] : current.filter((id) => id !== row.id)))}
            />
            <div className="min-w-0 flex-1">
              <p className="text-[14px]">{row.name}</p>
              <p className="truncate text-[12px] text-muted">{row.id} · {row.version}</p>
            </div>
            <Button size="sm" variant="outline" onClick={() => void install([row.id])}>
              instalar
            </Button>
          </div>
        ))}
      </div>
      {log && <pre className="mt-4 max-h-48 overflow-auto whitespace-pre-wrap text-[12px] text-muted">{log}</pre>}
    </div>
  )
}
