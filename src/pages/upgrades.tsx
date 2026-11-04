import { useState } from "react"
import { PageHeader } from "../components/page-header"
import { Button } from "../components/ui/button"
import { errorText, invoke, isTauri, onBob } from "../lib/api"
import { demoUpgrades } from "../lib/demo"
import type { WingetRow } from "../lib/types"
import { toast } from "../store"

export function UpgradesPage() {
  const [rows, setRows] = useState<WingetRow[]>(isTauri() ? [] : demoUpgrades)
  const [busy, setBusy] = useState(false)
  const [log, setLog] = useState("")
  const [error, setError] = useState("")

  async function load() {
    if (!isTauri()) {
      setRows(demoUpgrades)
      return
    }
    setBusy(true)
    setError("")
    try {
      setRows(await invoke<WingetRow[]>("winget_upgrades"))
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusy(false)
    }
  }

  async function upgrade(ids: string[]) {
    if (!isTauri()) {
      toast("no navegador o winget não atualiza")
      return
    }
    const stop = await onBob((event) => {
      if (event.type === "jobLog") setLog((current) => `${current}${event.line}\n`)
    })
    try {
      await invoke("winget_upgrade", { ids, all: ids.length > 1 })
      toast("atualização iniciada")
    } catch (err) {
      setError(errorText(err))
    } finally {
      window.setTimeout(stop, 1200)
    }
  }

  return (
    <div>
      <PageHeader title="atualizar" sub="o que o winget sabe que tem versão nova. dá para atualizar tudo ou só um." />
      <div className="mb-5 flex gap-2">
        <Button onClick={() => void load()} disabled={busy}>
          procurar atualizações
        </Button>
        <Button variant="outline" disabled={!rows.length} onClick={() => void upgrade(rows.map((row) => row.id))}>
          atualizar tudo
        </Button>
      </div>
      {error && <p className="mb-3 text-[13px] text-danger">{error}</p>}
      {!rows.length && !busy && <p className="text-[14px] text-muted">nada listado. se estiver tudo em dia, a lista fica vazia.</p>}
      <div className="flex flex-col gap-2">
        {rows.map((row) => (
          <div key={row.id} className="flex items-center gap-3 rounded-2xl border border-line px-4 py-3">
            <div className="min-w-0 flex-1">
              <p className="text-[14px]">{row.name}</p>
              <p className="text-[12px] text-muted">
                {row.version} → {row.available}
              </p>
            </div>
            <Button size="sm" variant="outline" onClick={() => void upgrade([row.id])}>
              atualizar
            </Button>
          </div>
        ))}
      </div>
      {log && <pre className="mt-4 max-h-48 overflow-auto whitespace-pre-wrap text-[12px] text-muted">{log}</pre>}
    </div>
  )
}
