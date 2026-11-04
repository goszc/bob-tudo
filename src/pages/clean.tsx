import { useState } from "react"
import { PageHeader } from "../components/page-header"
import { Button } from "../components/ui/button"
import { Checkbox } from "../components/ui/checkbox"
import { errorText, invoke, isTauri } from "../lib/api"
import { demoClean } from "../lib/demo"
import { bytes } from "../lib/format"
import type { CleanCategory } from "../lib/types"
import { ask, toast } from "../store"

export function CleanPage() {
  const [items, setItems] = useState<CleanCategory[]>(isTauri() ? [] : demoClean)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState("")
  const [log, setLog] = useState("")

  async function preview() {
    if (!isTauri()) {
      setItems(demoClean)
      return
    }
    setBusy(true)
    setError("")
    try {
      setItems(await invoke<CleanCategory[]>("clean_preview"))
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusy(false)
    }
  }

  function run() {
    const ids = items.filter((item) => item.checked && item.available).map((item) => item.id)
    if (!ids.length) {
      toast("marque ao menos uma categoria")
      return
    }
    const admin = items.some((item) => item.checked && item.needsAdmin)
    ask(
      "limpar agora",
      admin
        ? "parte disso pede a janela do windows para confirmar. temporários e caches são apagados de vez. a lixeira, se marcada, é esvaziada."
        : "temporários e caches são apagados de vez, sem ir para a lixeira.",
      "limpar",
      () => {
        void (async () => {
          if (!isTauri()) {
            toast("no navegador nada é apagado")
            return
          }
          setBusy(true)
          try {
            const result = await invoke<{ id: string; bytes: number; files: number; skipped: number }[]>("clean_run", { ids })
            const freed = result.reduce((sum, item) => sum + item.bytes, 0)
            const skipped = result.reduce((sum, item) => sum + item.skipped, 0)
            setLog(skipped ? `${skipped} itens foram pulados` : "")
            toast(`${bytes(freed)} liberados`)
            await preview()
          } catch (err) {
            setError(errorText(err))
          } finally {
            setBusy(false)
          }
        })()
      },
    )
  }

  const total = items.filter((item) => item.checked && item.available).reduce((sum, item) => sum + item.bytes, 0)

  return (
    <div>
      <PageHeader title="limpeza" sub="mede cada categoria antes de apagar. temporários e caches saem de vez. o mapa e os duplicados usam a lixeira." />
      <div className="mb-5 flex flex-wrap items-center gap-3">
        <Button onClick={() => void preview()} disabled={busy}>
          {items.length ? "medir de novo" : "medir"}
        </Button>
        <Button variant="outline" onClick={run} disabled={busy || !items.length}>
          limpar {total ? bytes(total) : ""}
        </Button>
        {busy && <span className="text-[13px] text-muted">trabalhando…</span>}
      </div>
      {error && <p className="mb-3 text-[13px] text-danger">{error}</p>}
      {!items.length && !busy && <p className="text-[14px] text-muted">ainda não há uma medição. aperte medir para ver o tamanho de cada categoria.</p>}
      <div className="flex flex-col gap-2">
        {items.map((item) => (
          <label key={item.id} className="flex items-start gap-3 rounded-2xl border border-line px-4 py-3">
            <Checkbox
              checked={item.checked}
              disabled={!item.available}
              label={item.title}
              onCheckedChange={(checked) => setItems((current) => current.map((row) => (row.id === item.id ? { ...row, checked } : row)))}
            />
            <span className="min-w-0 flex-1">
              <span className="flex items-center gap-2 text-[14px]">
                {item.title}
                {item.needsAdmin && <span className="rounded-full bg-soft px-2 py-0.5 text-[11px] text-muted">admin</span>}
                {!item.available && <span className="text-[12px] text-muted">não encontrado</span>}
              </span>
              <span className="mt-1 block text-[13px] leading-relaxed text-muted">{item.detail}</span>
            </span>
            <span className="shrink-0 text-[13px] text-muted">{item.measured ? `${bytes(item.bytes)} · ${new Intl.NumberFormat("pt-BR").format(item.files)}` : "tamanho variável"}</span>
          </label>
        ))}
      </div>
      {log && <pre className="mt-4 whitespace-pre-wrap text-[12px] text-muted">{log}</pre>}
    </div>
  )
}
