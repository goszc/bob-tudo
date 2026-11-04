import { useState } from "react"
import { PageHeader } from "../components/page-header"
import { Button } from "../components/ui/button"
import { Input } from "../components/ui/input"
import { errorText, invoke, isTauri } from "../lib/api"
import { demoApps, demoLeftovers } from "../lib/demo"
import { bytes } from "../lib/format"
import type { InstalledApp, Leftover } from "../lib/types"
import { ask, toast } from "../store"

export function UninstallPage() {
  const [apps, setApps] = useState<InstalledApp[]>(isTauri() ? [] : demoApps)
  const [query, setQuery] = useState("")
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState("")
  const [leftovers, setLeftovers] = useState<Leftover[]>([])
  const [picked, setPicked] = useState<InstalledApp | null>(null)

  async function load() {
    if (!isTauri()) {
      setApps(demoApps)
      return
    }
    setBusy(true)
    setError("")
    try {
      setApps(await invoke<InstalledApp[]>("apps_list"))
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusy(false)
    }
  }

  function uninstall(app: InstalledApp) {
    ask("desinstalar", `${app.name} será removido. depois dá para procurar o que sobrou.`, "desinstalar", () => {
      void (async () => {
        if (!isTauri()) {
          toast("no navegador nada é desinstalado")
          setPicked(app)
          setLeftovers(demoLeftovers)
          return
        }
        setBusy(true)
        try {
          await invoke("apps_uninstall", { id: app.id })
          setPicked(app)
          setLeftovers(await invoke<Leftover[]>("apps_leftovers", { name: app.name, location: app.location, registryId: app.id }))
          setApps((current) => current.filter((row) => row.id !== app.id))
        } catch (err) {
          setError(errorText(err))
        } finally {
          setBusy(false)
        }
      })()
    })
  }

  function sweep() {
    if (!picked) return
    ask("apagar sobras", "pastas e chaves de desinstalação encontradas. pastas do windows não entram.", "apagar", () => {
      void (async () => {
        if (!isTauri()) {
          toast("no navegador nada é apagado")
          setLeftovers([])
          return
        }
        const result = await invoke<{ bytes: number; files: number; skipped: number }>("apps_remove_leftovers", { paths: leftovers.map((item) => item.path) })
        toast(`${result.files} arquivos removidos`)
        setLeftovers([])
      })()
    })
  }

  const shown = apps.filter((app) => `${app.name} ${app.publisher}`.toLowerCase().includes(query.trim().toLowerCase()))

  return (
    <div>
      <PageHeader title="desinstalar" sub="programas do registro e do winget. depois da remoção, o bob tudo procura pastas e chaves que ficaram." />
      <div className="mb-4 flex gap-2">
        <Button onClick={() => void load()} disabled={busy}>
          listar programas
        </Button>
        <Input value={query} placeholder="filtrar" onChange={(event) => setQuery(event.target.value)} />
      </div>
      {error && <p className="mb-3 whitespace-pre-wrap text-[13px] text-danger">{error}</p>}
      {!shown.length && !busy && <p className="text-[14px] text-muted">nenhum programa na lista.</p>}
      <div className="flex flex-col gap-2">
        {shown.map((app) => (
          <div key={app.id} className="flex items-center gap-3 rounded-2xl border border-line px-4 py-3">
            <div className="min-w-0 flex-1">
              <p className="text-[14px]">{app.name}</p>
              <p className="truncate text-[12px] text-muted">
                {app.version} · {app.publisher || app.source} · {bytes(app.size)}
              </p>
            </div>
            <Button size="sm" variant="outline" onClick={() => uninstall(app)}>
              desinstalar
            </Button>
          </div>
        ))}
      </div>
      {!!leftovers.length && (
        <section className="mt-8">
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-[15px]">sobras de {picked?.name}</h2>
            <Button size="sm" onClick={sweep}>
              apagar sobras
            </Button>
          </div>
          {leftovers.map((item) => (
            <p key={item.path} className="truncate border-t border-line py-2 text-[13px] text-muted">
              {item.kind} · {item.path} · {bytes(item.bytes)}
            </p>
          ))}
        </section>
      )}
    </div>
  )
}
