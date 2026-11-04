import { useState } from "react"
import { PageHeader } from "../components/page-header"
import { Button } from "../components/ui/button"
import { errorText, invoke, isTauri } from "../lib/api"
import { demoChanges, demoPoints } from "../lib/demo"
import { when } from "../lib/format"
import type { Change, RestorePoint } from "../lib/types"
import { ask, toast } from "../store"

export function RestorePage() {
  const [points, setPoints] = useState<RestorePoint[]>(isTauri() ? [] : demoPoints)
  const [changes, setChanges] = useState<Change[]>(isTauri() ? [] : demoChanges)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState("")

  async function load() {
    if (!isTauri()) {
      setPoints(demoPoints)
      setChanges(demoChanges)
      return
    }
    setBusy(true)
    setError("")
    try {
      const snapshot = await invoke<{ points: RestorePoint[]; changes: Change[] }>("restore_list")
      setPoints(snapshot.points)
      setChanges(snapshot.changes)
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusy(false)
    }
  }

  async function create() {
    if (!isTauri()) {
      toast("no navegador o ponto não é criado")
      return
    }
    setBusy(true)
    try {
      toast(await invoke<string>("restore_create"))
      await load()
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusy(false)
    }
  }

  function undo(change: Change) {
    ask("desfazer esta mudança", change.summary, "desfazer", () => {
      void (async () => {
        if (!isTauri()) {
          toast("no navegador nada é desfeito")
          return
        }
        const warning = await invoke<string>("restore_undo", { id: change.id })
        if (warning) toast(warning)
        setChanges((current) => current.map((row) => (row.id === change.id ? { ...row, undone: true } : row)))
      })()
    })
  }

  function system(point: RestorePoint) {
    ask(
      "restaurar o windows",
      "o computador volta para este ponto e reinicia. arquivos seus em geral ficam, mas programas instalados depois podem sumir.",
      "restaurar e reiniciar",
      () => {
        void (async () => {
          if (!isTauri()) {
            toast("no navegador o windows não restaura")
            return
          }
          await invoke("restore_system", { sequence: point.sequence })
        })()
      },
    )
  }

  return (
    <div>
      <PageHeader title="restauração" sub="pontos do windows e o diário do que o bob tudo mudou. a primeira mudança do sistema em dez minutos cria um ponto." />
      <div className="mb-6 flex gap-2">
        <Button onClick={() => void load()} disabled={busy}>
          atualizar
        </Button>
        <Button variant="outline" onClick={() => void create()} disabled={busy}>
          criar ponto
        </Button>
      </div>
      {error && <p className="mb-3 text-[13px] text-danger">{error}</p>}
      <div className="grid gap-8 lg:grid-cols-2">
        <section>
          <h2 className="mb-3 text-[15px]">pontos do windows</h2>
          {!points.length && <p className="text-[14px] text-muted">nenhum ponto listado. a restauração do sistema precisa estar ligada.</p>}
          <div className="flex flex-col gap-2">
            {points.map((point) => (
              <div key={point.sequence} className="flex items-center gap-3 rounded-2xl border border-line px-4 py-3">
                <div className="min-w-0 flex-1">
                  <p className="text-[14px]">{point.description}</p>
                  <p className="text-[12px] text-muted">{when(point.when)}</p>
                </div>
                <Button size="sm" variant="outline" onClick={() => system(point)}>
                  restaurar
                </Button>
              </div>
            ))}
          </div>
        </section>
        <section>
          <h2 className="mb-3 text-[15px]">diário do bob tudo</h2>
          {!changes.length && <p className="text-[14px] text-muted">ainda não há mudanças registradas.</p>}
          <div className="flex flex-col gap-2">
            {changes.map((change) => (
              <div key={change.id} className="flex items-center gap-3 rounded-2xl border border-line px-4 py-3">
                <div className="min-w-0 flex-1">
                  <p className="text-[14px]">{change.summary}</p>
                  <p className="text-[12px] text-muted">{when(change.at)}{change.undone ? " · desfeito" : ""}</p>
                </div>
                {!change.undone && (
                  <Button size="sm" variant="ghost" onClick={() => undo(change)}>
                    desfazer
                  </Button>
                )}
              </div>
            ))}
          </div>
        </section>
      </div>
    </div>
  )
}
