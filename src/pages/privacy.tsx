import { useState } from "react"
import { PageHeader } from "../components/page-header"
import { Button } from "../components/ui/button"
import { Switch } from "../components/ui/switch"
import { errorText, invoke, isTauri } from "../lib/api"
import { demoPrivacy } from "../lib/demo"
import type { PrivacyItem } from "../lib/types"
import { toast } from "../store"

export function PrivacyPage() {
  const [items, setItems] = useState<PrivacyItem[]>(isTauri() ? [] : demoPrivacy)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState("")

  async function load() {
    if (!isTauri()) {
      setItems(demoPrivacy)
      return
    }
    setBusy(true)
    setError("")
    try {
      setItems(await invoke<PrivacyItem[]>("privacy_list"))
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusy(false)
    }
  }

  async function toggle(item: PrivacyItem, enabled: boolean) {
    if (!isTauri()) {
      setItems((current) => current.map((row) => (row.id === item.id ? { ...row, enabled } : row)))
      toast("no navegador o registro não muda")
      return
    }
    try {
      const warning = await invoke<string>("privacy_set", { id: item.id, enabled })
      setItems((current) => current.map((row) => (row.id === item.id ? { ...row, enabled } : row)))
      if (warning) toast(warning)
    } catch (err) {
      setError(errorText(err))
    }
  }

  return (
    <div>
      <PageHeader title="privacidade" sub="cada interruptor ligado é uma proteção. desligar devolve o valor que estava no registro." />
      <Button className="mb-5" onClick={() => void load()} disabled={busy}>
        {items.length ? "atualizar" : "ler ajustes"}
      </Button>
      {error && <p className="mb-3 text-[13px] text-danger">{error}</p>}
      {!items.length && !busy && <p className="text-[14px] text-muted">os interruptores aparecem depois da leitura.</p>}
      <div className="flex flex-col gap-2">
        {items.map((item) => (
          <div key={item.id} className="flex items-center gap-4 rounded-2xl border border-line px-4 py-3">
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2 text-[14px]">
                {item.title}
                {item.needsAdmin && <span className="text-[12px] text-muted">admin</span>}
              </div>
              <p className="text-[13px] text-muted">{item.detail}</p>
            </div>
            <Switch checked={item.enabled} label={item.title} onCheckedChange={(enabled) => void toggle(item, enabled)} />
          </div>
        ))}
      </div>
    </div>
  )
}
