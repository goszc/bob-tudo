import { useMemo, useState } from "react"
import { PageHeader } from "../components/page-header"
import { Button } from "../components/ui/button"
import { Input } from "../components/ui/input"
import { errorText, invoke, isTauri } from "../lib/api"
import { demoOss } from "../lib/demo"
import { stars } from "../lib/format"
import type { OssApp } from "../lib/types"
import { toast } from "../store"

export function OssPage() {
  const [query, setQuery] = useState("")
  const [catalog, setCatalog] = useState<OssApp[]>(isTauri() ? [] : demoOss)
  const [live, setLive] = useState<OssApp[]>([])
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState("")
  const [loaded, setLoaded] = useState(!isTauri())

  async function load() {
    if (!isTauri()) {
      setCatalog(demoOss)
      setLoaded(true)
      return
    }
    setBusy(true)
    setError("")
    try {
      setCatalog(await invoke<OssApp[]>("oss_catalog_cmd"))
      setLoaded(true)
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusy(false)
    }
  }

  async function search() {
    if (!query.trim()) return
    if (!isTauri()) {
      setLive(demoOss.filter((app) => app.name.includes(query.toLowerCase()) || app.description.includes(query.toLowerCase())))
      return
    }
    setBusy(true)
    setError("")
    try {
      setLive(await invoke<OssApp[]>("oss_search", { query }))
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusy(false)
    }
  }

  async function install(id: string) {
    if (!id) return
    if (!isTauri()) {
      toast("no navegador o winget não instala")
      return
    }
    await invoke("winget_install", { ids: [id] })
    toast("instalação iniciada")
  }

  const shown = useMemo(() => {
    const source = live.length ? live : catalog
    const text = query.trim().toLowerCase()
    if (!text || live.length) return source
    return source.filter((app) => `${app.name} ${app.description} ${app.category} ${app.alternativeTo}`.toLowerCase().includes(text))
  }, [catalog, live, query])

  const categories = [...new Set(shown.map((app) => app.category))]

  return (
    <div>
      <PageHeader title="código aberto" sub="aplicativos livres para o windows, com estrelas no github e instalação pelo winget quando existe um id." />
      <div className="mb-5 flex gap-2">
        <Input value={query} placeholder="buscar, inclusive alternative ao photoshop" onChange={(event) => setQuery(event.target.value)} onKeyDown={(event) => event.key === "Enter" && void search()} />
        {!loaded ? (
          <Button onClick={() => void load()} disabled={busy}>
            carregar
          </Button>
        ) : (
          <Button variant="outline" onClick={() => void search()} disabled={busy}>
            buscar no github
          </Button>
        )}
      </div>
      {error && <p className="mb-3 text-[13px] text-danger">{error}</p>}
      {!shown.length && <p className="text-[14px] text-muted">{loaded ? "nada com esse texto." : "carregue o catálogo para ver as sugestões."}</p>}
      {categories.map((category) => (
        <section key={category} className="mb-6">
          <h2 className="mb-2 text-[13px] text-muted">{category}</h2>
          <div className="grid gap-2">
            {shown
              .filter((app) => app.category === category)
              .map((app) => (
                <article key={`${app.repo}-${app.name}`} className="flex items-center gap-4 rounded-2xl border border-line px-4 py-3">
                  <div className="min-w-0 flex-1">
                    <p className="text-[14px]">{app.name}</p>
                    <p className="text-[13px] text-muted">{app.description}</p>
                    <p className="mt-1 text-[12px] text-muted">
                      {stars(app.stars)} estrelas{app.alternativeTo ? ` · ${app.alternativeTo}` : ""}
                    </p>
                  </div>
                  {app.wingetId ? (
                    <Button size="sm" variant="outline" onClick={() => void install(app.wingetId)}>
                      instalar
                    </Button>
                  ) : (
                    <span className="text-[12px] text-muted">sem winget</span>
                  )}
                </article>
              ))}
          </div>
        </section>
      ))}
    </div>
  )
}
