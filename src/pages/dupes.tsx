import { useEffect, useState } from "react"
import { PageHeader } from "../components/page-header"
import { Progress } from "../components/progress"
import { Button } from "../components/ui/button"
import { Input } from "../components/ui/input"
import { errorText, invoke, isTauri, onBob } from "../lib/api"
import { demoDupes } from "../lib/demo"
import { bytes } from "../lib/format"
import type { DupeGroup } from "../lib/types"
import { ask, toast } from "../store"

export function DupesPage() {
  const [root, setRoot] = useState(isTauri() ? "" : "C:\\Users\\ana")
  const [groups, setGroups] = useState<DupeGroup[]>(isTauri() ? [] : demoDupes)
  const [busy, setBusy] = useState(false)
  const [phase, setPhase] = useState("")
  const [error, setError] = useState("")
  const [kind, setKind] = useState<"tudo" | "igual" | "imagem">("tudo")

  useEffect(() => {
    if (!isTauri()) return
    let stop = () => {}
    void onBob((event) => {
      if (event.type === "dupeProgress") setPhase(`${event.phase} · ${event.done} de ${event.total}`)
    }).then((unlisten) => {
      stop = unlisten
    })
    return () => stop()
  }, [])

  async function scan() {
    if (!root.trim()) {
      toast("informe uma pasta")
      return
    }
    if (!isTauri()) {
      setGroups(demoDupes)
      return
    }
    setBusy(true)
    setError("")
    try {
      setGroups(await invoke<DupeGroup[]>("find_duplicates", { paths: [root], images: true }))
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusy(false)
      setPhase("")
    }
  }

  function trash(group: DupeGroup) {
    const paths = group.files.filter((file) => !file.keep).map((file) => file.path)
    if (!paths.length) return
    ask("mandar cópias para a lixeira", `${paths.length} arquivo(s) saem. o que está marcado para ficar permanece.`, "excluir", () => {
      void (async () => {
        if (!isTauri()) {
          toast("no navegador nada é apagado")
          return
        }
        const report = await invoke<{ deleted: string[]; failed: { error: string }[] }>("delete_to_trash", { paths })
        toast(report.failed[0] ? report.failed[0].error : "foi para a lixeira")
        setGroups((current) => current.filter((item) => item.id !== group.id))
      })()
    })
  }

  const shown = groups.filter((group) => kind === "tudo" || group.kind === kind)
  const waste = shown.reduce((sum, group) => sum + group.files.filter((file) => !file.keep).reduce((inner, file) => inner + file.size, 0), 0)

  return (
    <div>
      <PageHeader title="duplicados" sub="arquivos iguais pelo conteúdo, e fotos quase iguais na mesma pasta. as cópias vão para a lixeira." />
      <div className="mb-4 flex flex-wrap gap-2">
        <Input className="max-w-md" value={root} placeholder="pasta para analisar" onChange={(event) => setRoot(event.target.value)} />
        <Button onClick={() => void scan()} disabled={busy}>
          analisar
        </Button>
      </div>
      {busy && (
        <div className="mb-4">
          <Progress value={0} active />
          <p className="mt-2 text-[13px] text-muted">{phase || "comparando…"}</p>
        </div>
      )}
      {error && <p className="mb-3 text-[13px] text-danger">{error}</p>}
      {!!shown.length && (
        <div className="mb-4 flex flex-wrap items-center gap-2 text-[13px] text-muted">
          <span>{shown.length} grupos · {bytes(waste)} em cópias</span>
          {(["tudo", "igual", "imagem"] as const).map((item) => (
            <button key={item} type="button" className={kind === item ? "text-ink" : ""} onClick={() => setKind(item)}>
              {item}
            </button>
          ))}
        </div>
      )}
      {!shown.length && !busy && <p className="text-[14px] text-muted">nenhum grupo para mostrar. analise uma pasta com arquivos repetidos.</p>}
      <div className="flex flex-col gap-3">
        {shown.map((group) => (
          <section key={group.id} className="rounded-2xl border border-line p-4">
            <div className="mb-3 flex items-center justify-between gap-3">
              <div>
                <p className="text-[14px]">{group.kind === "imagem" ? "fotos parecidas" : "arquivos iguais"}</p>
                <p className="text-[12px] text-muted">{bytes(group.bytes)} cada cópia cheia</p>
              </div>
              <Button size="sm" variant="outline" onClick={() => trash(group)}>
                lixeira nas cópias
              </Button>
            </div>
            {group.files.map((file) => (
              <div key={file.path} className="flex items-center gap-3 border-t border-line py-2 text-[13px]">
                <span className={file.keep ? "text-ink" : "text-muted"}>{file.keep ? "fica" : "sai"}</span>
                <span className="min-w-0 flex-1 truncate">{file.path}</span>
                {file.distance ? <span className="text-muted">distância {file.distance}</span> : null}
                <span className="text-muted">{bytes(file.size)}</span>
              </div>
            ))}
          </section>
        ))}
      </div>
    </div>
  )
}
