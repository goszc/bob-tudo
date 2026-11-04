import { useState } from "react"
import { PageHeader } from "../components/page-header"
import { Button } from "../components/ui/button"
import { Switch } from "../components/ui/switch"
import { errorText, invoke, isTauri } from "../lib/api"
import { demoStartup } from "../lib/demo"
import type { StartupItem } from "../lib/types"
import { toast } from "../store"

export function StartupPage() {
  const [items, setItems] = useState<StartupItem[]>(isTauri() ? [] : demoStartup)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState("")

  async function load() {
    if (!isTauri()) {
      setItems(demoStartup)
      return
    }
    setBusy(true)
    setError("")
    try {
      setItems(await invoke<StartupItem[]>("startup_list"))
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusy(false)
    }
  }

  async function toggle(item: StartupItem, enabled: boolean) {
    if (!isTauri()) {
      setItems((current) => current.map((row) => (row.id === item.id ? { ...row, enabled } : row)))
      toast("no navegador o windows não muda")
      return
    }
    setError("")
    try {
      await invoke("startup_set", { id: item.id, enabled })
      setItems((current) => current.map((row) => (row.id === item.id ? { ...row, enabled } : row)))
    } catch (err) {
      setError(errorText(err))
    }
  }

  return (
    <div>
      <PageHeader title="inicialização" sub="o que abre com o windows. impacto alto é o que pesa no login. serviços do próprio windows ficam de fora." />
      <Button className="mb-5" onClick={() => void load()} disabled={busy}>
        {items.length ? "atualizar lista" : "listar"}
      </Button>
      {error && <p className="mb-3 text-[13px] text-danger">{error}</p>}
      {!items.length && !busy && <p className="text-[14px] text-muted">a lista ainda está vazia.</p>}
      <div className="flex flex-col gap-2">
        {items.map((item) => (
          <div key={item.id} className="flex items-center gap-4 rounded-2xl border border-line px-4 py-3">
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2 text-[14px]">
                {item.name}
                <span className="text-[12px] text-muted">{item.impact}</span>
                {item.needsAdmin && <span className="text-[12px] text-muted">admin</span>}
              </div>
              <p className="truncate text-[12px] text-muted">{item.source} · {item.command}</p>
            </div>
            <Switch checked={item.enabled} disabled={!item.canToggle} label={item.name} onCheckedChange={(enabled) => void toggle(item, enabled)} />
          </div>
        ))}
      </div>
    </div>
  )
}
