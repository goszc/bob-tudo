import { useEffect, useRef, useState } from "react"
import * as Dialog from "@radix-ui/react-dialog"
import { PageHeader } from "../components/page-header"
import { Segmented } from "../components/segmented"
import { Button } from "../components/ui/button"
import { Checkbox } from "../components/ui/checkbox"
import { Input } from "../components/ui/input"
import { demoCollections, demoDetail, demoHits } from "../lib/catalog-demo"
import { errorText, invoke, isTauri } from "../lib/api"
import { bytes, stars } from "../lib/format"
import type { CatalogCollection, CatalogDetail, CatalogHit, CatalogOffer, ManagerInfo } from "../lib/types"
import { toast, useApp } from "../store"
import { cn } from "../lib/cn"

type View = "grid" | "list"
type Tri = "todos" | "sim" | "nao"

type Query = {
  text: string
  category: string
  openSource: Tri
  minStars: number
  recent: boolean
  installed: Tri
  kind: Tri
  sort: string
  section: string
  collectionId: string
}

const EMPTY: Query = {
  text: "",
  category: "",
  openSource: "todos",
  minStars: 0,
  recent: false,
  installed: "todos",
  kind: "todos",
  sort: "relevancia",
  section: "",
  collectionId: "",
}

const ORDER = ["winget", "scoop", "chocolatey", "github"]

function managerOf(source: string) {
  return source === "winget" || source === "scoop" || source === "chocolatey" ? source : ""
}

function ready(source: string, managers: ManagerInfo[]) {
  const id = managerOf(source)
  if (!id) return true
  return managers.some((item) => item.id === id && item.installed)
}

function readyOffers(hit: CatalogHit, managers: ManagerInfo[]) {
  return [...hit.offers].filter((offer) => ready(offer.source, managers)).sort((a, b) => ORDER.indexOf(a.source) - ORDER.indexOf(b.source))
}

function firstBlocked(hit: CatalogHit, managers: ManagerInfo[]) {
  for (const source of ORDER) {
    if (hit.offers.some((offer) => offer.source === source) && !ready(source, managers)) return source
  }
  return ""
}

function ago(ms: number) {
  if (!ms) return ""
  const days = Math.max(0, Math.round((Date.now() - ms) / 86_400_000))
  if (days < 1) return "hoje"
  if (days < 30) return `há ${days} dias`
  return `há ${Math.round(days / 30)} meses`
}

function initialView(): View {
  const query = new URLSearchParams(location.search).get("view")
  if (query === "list" || query === "grid") return query
  return localStorage.getItem("bob-tudo-catalog-view") === "list" ? "list" : "grid"
}

function initialApp() {
  const value = new URLSearchParams(location.search).get("app")
  if (!value) return ""
  if (value === "gimp") return "wg:gimp.gimp"
  return value
}

function triBool(value: Tri): boolean | null {
  if (value === "sim") return true
  if (value === "nao") return false
  return null
}

export function CatalogPage() {
  const managers = useApp().managers
  const updating = useApp().updating.includes("catalog")
  const tauri = isTauri()
  const [query, setQuery] = useState<Query>(EMPTY)
  const [view, setView] = useState<View>(initialView)
  const [library, setLibrary] = useState<CatalogHit[]>(() => (tauri ? [] : demoHits.map((hit) => ({ ...hit, offers: hit.offers.map((offer) => ({ ...offer })) }))))
  const [collections, setCollections] = useState<CatalogCollection[]>(() => (tauri ? [] : demoCollections.map((item) => ({ ...item, apps: [...item.apps] }))))
  const [hits, setHits] = useState<CatalogHit[]>(() => (tauri ? [] : filterDemo(demoHits, demoCollections, EMPTY)))
  const [total, setTotal] = useState(() => (tauri ? 0 : demoHits.length))
  const [categories, setCategories] = useState<string[]>(() => (tauri ? [] : Array.from(new Set(demoHits.map((hit) => hit.category))).sort()))
  const [selected, setSelected] = useState(initialApp)
  const [detail, setDetail] = useState<CatalogDetail | null>(null)
  const [picked, setPicked] = useState<string[]>([])
  const [picker, setPicker] = useState<CatalogHit | null>(null)
  const [collectionName, setCollectionName] = useState("")

  useEffect(() => {
    localStorage.setItem("bob-tudo-catalog-view", view)
  }, [view])

  useEffect(() => {
    if (!tauri) return
    void invoke<CatalogCollection[]>("catalog_collections").then(setCollections).catch(() => {})
  }, [tauri])

  useEffect(() => {
    if (tauri) return
    const filtered = filterDemo(library, collections, query)
    setHits(filtered)
    setTotal(filtered.length)
    setCategories(Array.from(new Set(library.map((hit) => hit.category))).sort())
  }, [tauri, library, collections, query])

  useEffect(() => {
    if (!tauri) return
    const handle = window.setTimeout(() => {
      void invoke<{ total: number; hits: CatalogHit[]; categories: string[] }>("catalog_search", {
        query: {
          text: query.text,
          category: query.category,
          openSource: triBool(query.openSource),
          minStars: query.minStars,
          recentDays: query.recent ? 180 : 0,
          installed: triBool(query.installed),
          portable: query.kind === "todos" ? null : query.kind === "sim",
          sort: query.sort,
          offset: 0,
          limit: 80,
          section: query.section,
          collectionId: query.collectionId,
          nowMs: Date.now(),
        },
      })
        .then((page) => {
          setHits(page.hits)
          setTotal(page.total)
          setCategories(page.categories)
        })
        .catch((err) => toast(errorText(err)))
    }, 180)
    return () => window.clearTimeout(handle)
  }, [tauri, query])

  useEffect(() => {
    if (!selected) {
      setDetail(null)
      return
    }
    if (!tauri) {
      setDetail(demoDetail(selected, library))
      return
    }
    void invoke<CatalogDetail>("catalog_detail", { id: selected })
      .then(setDetail)
      .catch((err) => toast(errorText(err)))
  }, [selected, tauri, library])

  function patchQuery(next: Partial<Query>) {
    setQuery((current) => ({ ...current, ...next }))
  }

  function chooseView(next: View) {
    setView(next)
  }

  async function installOffers(items: { hit: CatalogHit; offer: CatalogOffer; assetUrl?: string; assetName?: string }[]) {
    if (!items.length) {
      toast("nenhuma fonte pronta para instalar")
      return
    }
    if (!tauri) {
      toast("no navegador a instalação não começa")
      return
    }
    try {
      await invoke("catalog_install", {
        requests: items.map((item) => ({
          source: item.offer.source,
          sourceId: item.offer.sourceId,
          version: item.offer.version,
          assetUrl: item.assetUrl ?? "",
          assetName: item.assetName ?? "",
        })),
      })
    } catch (err) {
      toast(errorText(err))
    }
  }

  async function installManager(id: string) {
    if (!tauri) {
      toast(`no navegador o ${id} não é instalado`)
      return
    }
    try {
      await invoke("manager_install", { id })
      toast(id === "chocolatey" ? "o windows vai pedir permissão" : `instalando o ${id}`)
    } catch (err) {
      toast(errorText(err))
    }
  }

  function installHit(hit: CatalogHit) {
    const offers = readyOffers(hit, managers)
    if (offers.length > 1) {
      setPicker(hit)
      return
    }
    if (offers.length === 1) {
      void installOffers([{ hit, offer: offers[0] }])
      return
    }
    const blocked = firstBlocked(hit, managers)
    if (blocked) void installManager(blocked)
  }

  function installPicked() {
    const items = picked
      .map((id) => hits.find((hit) => hit.id === id) ?? library.find((hit) => hit.id === id))
      .filter((hit): hit is CatalogHit => !!hit)
      .map((hit) => {
        const offer = readyOffers(hit, managers)[0]
        return offer ? { hit, offer } : null
      })
      .filter((item): item is { hit: CatalogHit; offer: CatalogOffer } => !!item)
    void installOffers(items)
    setPicked([])
  }

  async function toggleFavorite(hit: CatalogHit) {
    const on = !hit.favorite
    if (tauri) {
      try {
        await invoke("catalog_favorite", { id: hit.id, on })
      } catch (err) {
        toast(errorText(err))
        return
      }
      setQuery((current) => ({ ...current }))
    }
    setLibrary((current) => current.map((item) => (item.id === hit.id ? { ...item, favorite: on } : item)))
    setDetail((current) => (current && current.id === hit.id ? { ...current, favorite: on } : current))
  }

  async function toggleWatch(hit: CatalogHit) {
    const on = !hit.watched
    if (tauri) {
      try {
        await invoke("catalog_watch", { id: hit.id, version: hit.latestVersion, on })
      } catch (err) {
        toast(errorText(err))
        return
      }
    }
    setLibrary((current) => current.map((item) => (item.id === hit.id ? { ...item, watched: on } : item)))
    setDetail((current) => (current && current.id === hit.id ? { ...current, watched: on } : current))
    if (!tauri) toast(on ? "aviso anotado para a próxima abertura" : "aviso removido")
  }

  async function createCollection() {
    const name = collectionName.trim()
    if (!name) return
    if (tauri) {
      try {
        const created = await invoke<CatalogCollection>("catalog_create_collection", { name })
        setCollections((current) => [...current, created])
      } catch (err) {
        toast(errorText(err))
      }
    } else {
      setCollections((current) => [...current, { id: name.toLowerCase().replace(/\s+/g, "-"), name, preset: false, apps: [] }])
    }
    setCollectionName("")
  }

  async function toggleCollection(collectionId: string, appId: string, on: boolean) {
    if (tauri) {
      try {
        await invoke("catalog_collection_app", { collectionId, appId, on })
      } catch (err) {
        toast(errorText(err))
        return
      }
    }
    setCollections((current) =>
      current.map((item) =>
        item.id === collectionId ? { ...item, apps: on ? Array.from(new Set([...item.apps, appId])) : item.apps.filter((id) => id !== appId) } : item,
      ),
    )
  }

  const sections = [
    { id: "", label: "tudo" },
    { id: "trending", label: "em alta" },
    { id: "stars", label: "mais estrelados" },
    { id: "alternatives", label: "alternativa a app pago" },
    { id: "favorites", label: "favoritos" },
  ]

  return (
    <div className={cn("flex flex-col", selected ? "" : "h-[calc(100dvh-4rem)]")}>
      <PageHeader title="catálogo" sub="winget, github, scoop e chocolatey no mesmo lugar. o que não está instalado fica visível, com o botão para colocar o gerenciador." />
      {updating && <p className="mb-4 text-[13px] text-muted">atualizando o catálogo</p>}
      {detail ? (
        <Detail
          detail={detail}
          managers={managers}
          collections={collections}
          onBack={() => setSelected("")}
          onInstall={() => installHit(detail)}
          onFavorite={() => void toggleFavorite(detail)}
          onWatch={() => void toggleWatch(detail)}
          onOpen={(id) => setSelected(id)}
          onManager={(id) => void installManager(id)}
          onOffer={(offer, assetUrl, assetName) => void installOffers([{ hit: detail, offer, assetUrl, assetName }])}
          onCollection={(collectionId, on) => void toggleCollection(collectionId, detail.id, on)}
        />
      ) : (
        <>
          <div className="mb-4 flex flex-wrap items-center gap-2">
            {sections.map((section) => (
              <button
                key={section.id || "tudo"}
                type="button"
                onClick={() => patchQuery({ section: section.id, collectionId: "" })}
                className={cn(
                  "h-8 rounded-full border px-3 text-[13px]",
                  query.section === section.id && !query.collectionId ? "border-fill bg-fill text-onfill" : "border-line bg-surface",
                )}
              >
                {section.label}
              </button>
            ))}
            {collections.map((collection) => (
              <button
                key={collection.id}
                type="button"
                onClick={() => patchQuery({ section: "collection", collectionId: collection.id })}
                className={cn(
                  "h-8 rounded-full border px-3 text-[13px]",
                  query.collectionId === collection.id ? "border-fill bg-fill text-onfill" : "border-line bg-surface",
                )}
              >
                {collection.name}
              </button>
            ))}
          </div>
          <div className="mb-4 flex flex-wrap items-center gap-2">
            <Input className="max-w-[240px]" placeholder="buscar" value={query.text} onChange={(event) => patchQuery({ text: event.target.value })} />
            <Select label="categoria" value={query.category} onChange={(category) => patchQuery({ category })} options={[{ value: "", label: "categoria" }, ...categories.map((item) => ({ value: item, label: item }))]} />
            <Select
              label="estrelas"
              value={String(query.minStars)}
              onChange={(value) => patchQuery({ minStars: Number(value) })}
              options={[
                { value: "0", label: "estrelas" },
                { value: "100", label: "100+" },
                { value: "1000", label: "1 mil+" },
                { value: "10000", label: "10 mil+" },
              ]}
            />
            <Select
              label="ordenar"
              value={query.sort}
              onChange={(sort) => patchQuery({ sort })}
              options={[
                { value: "relevancia", label: "relevância" },
                { value: "estrelas", label: "estrelas" },
                { value: "atualizacao", label: "atualização" },
                { value: "nome", label: "nome" },
              ]}
            />
            <button type="button" onClick={() => patchQuery({ recent: !query.recent })} className={cn("h-10 rounded-full border px-3 text-[13px]", query.recent ? "border-fill bg-fill text-onfill" : "border-line bg-surface")}>
              atualizado recentemente
            </button>
            <Segmented value={query.openSource} onChange={(openSource) => patchQuery({ openSource })} options={[{ value: "todos", label: "todos" }, { value: "sim", label: "livre" }, { value: "nao", label: "proprietário" }]} />
            <Segmented value={query.installed} onChange={(installed) => patchQuery({ installed })} options={[{ value: "todos", label: "qualquer" }, { value: "sim", label: "instalado" }, { value: "nao", label: "não instalado" }]} />
            <Segmented value={query.kind} onChange={(kind) => patchQuery({ kind })} options={[{ value: "todos", label: "formato" }, { value: "sim", label: "portátil" }, { value: "nao", label: "instalador" }]} />
            <div className="ml-auto">
              <Segmented value={view} onChange={chooseView} options={[{ value: "grid", label: "grade" }, { value: "list", label: "lista" }]} />
            </div>
          </div>
          <div className="mb-3 flex items-center gap-3">
            <p className="text-[13px] text-muted">{total} apps</p>
            {picked.length > 0 && (
              <>
                <Button size="sm" onClick={installPicked}>
                  instalar {picked.length}
                </Button>
                <Button size="sm" variant="ghost" onClick={() => setPicked([])}>
                  limpar
                </Button>
              </>
            )}
            <div className="ml-auto flex items-center gap-2">
              <Input className="h-8 w-36" placeholder="nova coleção" value={collectionName} onChange={(event) => setCollectionName(event.target.value)} />
              <Button size="sm" variant="outline" onClick={() => void createCollection()}>
                criar
              </Button>
            </div>
          </div>
          <Results
            hits={hits}
            view={view}
            managers={managers}
            picked={picked}
            onPick={(id, on) => setPicked((current) => (on ? [...current, id] : current.filter((item) => item !== id)))}
            onOpen={setSelected}
            onInstall={installHit}
          />
        </>
      )}
      <Dialog.Root open={!!picker} onOpenChange={(open) => !open && setPicker(null)}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-40 bg-ink/25" />
          <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-[min(460px,calc(100%-2rem))] -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-line bg-surface p-6">
            <Dialog.Title className="text-[16px] font-medium tracking-tight">instalar {picker?.name}</Dialog.Title>
            <Dialog.Description className="mt-2 text-[14px] text-muted">escolha a fonte. a que não tem gerenciador pode ser instalada daqui.</Dialog.Description>
            <div className="mt-4 flex flex-col gap-2">
              {picker?.offers.map((offer) => {
                const blocked = managerOf(offer.source) && !ready(offer.source, managers)
                return (
                  <div key={`${offer.source}-${offer.sourceId}`} className="flex items-center gap-3 rounded-xl border border-line px-3 py-2">
                    <div className="min-w-0 flex-1">
                      <p className="text-[14px]">{offer.source}</p>
                      <p className="truncate text-[12px] text-muted">
                        {offer.version} · {offer.portable ? "portátil" : "instalador"}
                      </p>
                    </div>
                    {blocked ? (
                      <Button size="sm" variant="outline" onClick={() => void installManager(managerOf(offer.source))}>
                        instalar o {managerOf(offer.source)}
                      </Button>
                    ) : (
                      <Button
                        size="sm"
                        onClick={() => {
                          void installOffers([{ hit: picker, offer }])
                          setPicker(null)
                        }}
                      >
                        instalar
                      </Button>
                    )}
                  </div>
                )
              })}
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    </div>
  )
}

function Results({
  hits,
  view,
  managers,
  picked,
  onPick,
  onOpen,
  onInstall,
}: {
  hits: CatalogHit[]
  view: View
  managers: ManagerInfo[]
  picked: string[]
  onPick: (id: string, on: boolean) => void
  onOpen: (id: string) => void
  onInstall: (hit: CatalogHit) => void
}) {
  const scroller = useRef<HTMLDivElement>(null)
  const [scroll, setScroll] = useState(0)
  const [size, setSize] = useState({ width: 900, height: 520 })
  useEffect(() => {
    const node = scroller.current
    if (!node) return
    const measure = () => setSize({ width: node.clientWidth || 900, height: node.clientHeight || 520 })
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(node)
    return () => observer.disconnect()
  }, [])
  const columns = view === "list" ? 1 : size.width < 640 ? 1 : size.width < 980 ? 2 : 3
  const rowHeight = view === "list" ? 84 : 236
  const rows = Math.ceil(hits.length / columns)
  const start = Math.max(0, Math.floor(scroll / rowHeight) - 1)
  const end = Math.min(rows, start + Math.ceil(size.height / rowHeight) + 3)
  const slice: CatalogHit[][] = []
  for (let row = start; row < end; row += 1) slice.push(hits.slice(row * columns, row * columns + columns))
  return (
    <div ref={scroller} className="min-h-0 flex-1 overflow-auto pr-1" onScroll={(event) => setScroll(event.currentTarget.scrollTop)}>
      <div style={{ height: rows * rowHeight, position: "relative" }}>
        <div style={{ transform: `translateY(${start * rowHeight}px)` }} className="flex flex-col gap-3">
          {slice.map((row, index) => (
            <div key={start + index} className={cn("grid gap-3", view === "list" ? "grid-cols-1" : columns === 3 ? "grid-cols-3" : columns === 2 ? "grid-cols-2" : "grid-cols-1")} style={{ minHeight: rowHeight - 12 }}>
              {row.map((hit) => (
                <Card key={hit.id} hit={hit} managers={managers} compact={view === "list"} picked={picked.includes(hit.id)} onPick={(on) => onPick(hit.id, on)} onOpen={() => onOpen(hit.id)} onInstall={() => onInstall(hit)} />
              ))}
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}

function Card({
  hit,
  managers,
  compact,
  picked,
  onPick,
  onOpen,
  onInstall,
}: {
  hit: CatalogHit
  managers: ManagerInfo[]
  compact: boolean
  picked: boolean
  onPick: (on: boolean) => void
  onOpen: () => void
  onInstall: () => void
}) {
  const offers = readyOffers(hit, managers)
  const blocked = firstBlocked(hit, managers)
  const label = offers.length > 1 ? "escolher fonte" : offers.length === 1 ? "instalar" : blocked ? `instalar o ${blocked}` : "ver"
  return (
    <article className={cn("flex rounded-2xl border border-line bg-surface p-4", compact ? "items-center gap-4" : "flex-col")}>
      <div className={cn("flex min-w-0 items-start gap-3", compact ? "flex-1" : "")}>
        <Checkbox checked={picked} label={`selecionar ${hit.name}`} onCheckedChange={onPick} />
        <button type="button" onClick={onOpen} className="flex min-w-0 flex-1 items-start gap-3 text-left">
          <Avatar name={hit.name} icon={hit.icon} />
          <span className="min-w-0">
            <span className="block truncate text-[15px]">{hit.name}</span>
            <span className="mt-1 line-clamp-2 block text-[13px] leading-relaxed text-muted">{hit.description}</span>
          </span>
        </button>
      </div>
      <div className={cn("flex items-center gap-2", compact ? "" : "mt-4")}>
        <div className="flex min-w-0 flex-1 flex-wrap gap-1">
          {hit.offers.map((offer) => (
            <span key={`${offer.source}-${offer.sourceId}`} className={cn("rounded-full px-2 py-0.5 text-[11px]", ready(offer.source, managers) ? "bg-soft text-ink" : "bg-soft text-muted line-through")}>
              {offer.source}
            </span>
          ))}
        </div>
        <Button size="sm" variant={offers.length ? "default" : "outline"} onClick={onInstall}>
          {label}
        </Button>
      </div>
      {!compact && (
        <p className="mt-3 text-[12px] text-muted">
          {stars(hit.stars)} estrelas · {hit.latestVersion || "—"} · {bytes(hit.size)} · {hit.license || "—"}
          {hit.installed ? " · instalado" : ""}
          {ago(hit.updatedMs) ? ` · ${ago(hit.updatedMs)}` : ""}
        </p>
      )}
    </article>
  )
}

function Detail({
  detail,
  managers,
  collections,
  onBack,
  onInstall,
  onFavorite,
  onWatch,
  onOpen,
  onManager,
  onOffer,
  onCollection,
}: {
  detail: CatalogDetail
  managers: ManagerInfo[]
  collections: CatalogCollection[]
  onBack: () => void
  onInstall: () => void
  onFavorite: () => void
  onWatch: () => void
  onOpen: (id: string) => void
  onManager: (id: string) => void
  onOffer: (offer: CatalogOffer, assetUrl?: string, assetName?: string) => void
  onCollection: (collectionId: string, on: boolean) => void
}) {
  const offers = readyOffers(detail, managers)
  const blocked = firstBlocked(detail, managers)
  return (
    <div>
      <button type="button" onClick={onBack} className="mb-4 text-[13px] text-muted hover:text-ink">
        voltar ao catálogo
      </button>
      <div className="mb-6 flex flex-wrap items-start gap-4">
        <Avatar name={detail.name} icon={detail.icon} />
        <div className="min-w-0 flex-1">
          <h2 className="text-[28px] font-medium tracking-[-0.04em]">{detail.name}</h2>
          <p className="mt-2 max-w-[52ch] text-[14px] leading-relaxed text-muted">{detail.description}</p>
          <p className="mt-2 text-[13px] text-muted">
            {stars(detail.stars)} estrelas · {detail.latestVersion || "sem versão"} · {bytes(detail.size)} · {detail.license || "sem licença"}
            {detail.openSource ? " · código aberto" : ""}
            {ago(detail.updatedMs) ? ` · ${ago(detail.updatedMs)}` : ""}
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button onClick={onInstall}>{offers.length > 1 ? "escolher fonte" : offers.length === 1 ? "instalar" : blocked ? `instalar o ${blocked}` : "sem fonte"}</Button>
          <Button variant="outline" onClick={onFavorite}>
            {detail.favorite ? "nos favoritos" : "favoritar"}
          </Button>
          <Button variant="outline" onClick={onWatch}>
            {detail.watched ? "avisando versão nova" : "avisar versão nova"}
          </Button>
        </div>
      </div>
      <div className="mb-6 flex flex-wrap gap-3 text-[13px]">
        {detail.homepage && (
          <a className="text-ink underline decoration-line underline-offset-4" href={detail.homepage} target="_blank" rel="noreferrer">
            site
          </a>
        )}
        {detail.repo && (
          <a className="text-ink underline decoration-line underline-offset-4" href={`https://github.com/${detail.repo}`} target="_blank" rel="noreferrer">
            github
          </a>
        )}
        {detail.alternativeTo && <span className="text-muted">{detail.alternativeTo}</span>}
      </div>
      {detail.images.length > 0 && (
        <div className="mb-6 grid grid-cols-3 gap-3">
          {detail.images.map((src) => (
            <img key={src} src={src} alt="" loading="lazy" className="h-36 w-full rounded-2xl border border-line object-cover" />
          ))}
        </div>
      )}
      <div className="grid gap-6 lg:grid-cols-[minmax(0,1.3fr)_minmax(280px,0.7fr)]">
        <section className="rounded-2xl border border-line p-5">
          <h3 className="mb-3 text-[13px] text-muted">readme</h3>
          <div className="prose-catalog text-[14px] leading-relaxed" dangerouslySetInnerHTML={{ __html: detail.readmeHtml || "<p>sem readme em cache.</p>" }} />
        </section>
        <section className="rounded-2xl border border-line p-5">
          <h3 className="mb-3 text-[13px] text-muted">versões</h3>
          <div className="flex flex-col gap-4">
            {detail.releases.length === 0 && <p className="text-[13px] text-muted">nenhuma versão em cache.</p>}
            {detail.releases.map((release) => (
              <div key={release.version}>
                <p className="text-[14px]">{release.version}</p>
                <p className="mt-1 text-[13px] text-muted">{release.notes}</p>
                <div className="mt-2 flex flex-wrap gap-2">
                  {release.url && (
                    <a className="text-[13px] underline decoration-line underline-offset-4" href={release.url} target="_blank" rel="noreferrer">
                      abrir a versão
                    </a>
                  )}
                  {release.assets.map((asset) => (
                    <Button key={asset.name} size="sm" variant="outline" onClick={() => onOffer({ source: "github", sourceId: detail.repo, version: release.version, size: asset.size, portable: asset.kind === "zip", installer: asset.kind === "exe" || asset.kind === "msi" }, asset.url, asset.name)}>
                      baixar {asset.name}
                    </Button>
                  ))}
                  {detail.wingetId && (
                    <Button size="sm" variant="outline" disabled={!ready("winget", managers)} onClick={() => onOffer({ source: "winget", sourceId: detail.wingetId, version: release.version, size: 0, portable: false, installer: true })}>
                      {ready("winget", managers) ? `winget ${release.version}` : "winget ausente"}
                    </Button>
                  )}
                </div>
              </div>
            ))}
          </div>
          <h3 className="mb-2 mt-6 text-[13px] text-muted">fontes</h3>
          <div className="flex flex-col gap-2">
            {detail.offers.map((offer) => (
              <div key={`${offer.source}-${offer.sourceId}`} className="flex items-center justify-between gap-2 text-[13px]">
                <span className={ready(offer.source, managers) ? "" : "text-muted"}>{offer.source}</span>
                {ready(offer.source, managers) ? (
                  <button type="button" className="text-ink underline decoration-line underline-offset-4" onClick={() => onOffer(offer)}>
                    instalar
                  </button>
                ) : (
                  <button type="button" className="text-ink underline decoration-line underline-offset-4" onClick={() => onManager(managerOf(offer.source))}>
                    instalar o {managerOf(offer.source)}
                  </button>
                )}
              </div>
            ))}
          </div>
        </section>
      </div>
      <section className="mt-6">
        <h3 className="mb-3 text-[13px] text-muted">coleções</h3>
        <div className="flex flex-wrap gap-2">
          {collections.map((collection) => {
            const on = collection.apps.includes(detail.id)
            return (
              <button key={collection.id} type="button" onClick={() => onCollection(collection.id, !on)} className={cn("h-8 rounded-full border px-3 text-[13px]", on ? "border-fill bg-fill text-onfill" : "border-line bg-surface")}>
                {on ? collection.name : `+ ${collection.name}`}
              </button>
            )
          })}
        </div>
      </section>
      {detail.alternatives.length > 0 && (
        <section className="mt-6">
          <h3 className="mb-3 text-[13px] text-muted">parecidos</h3>
          <div className="grid gap-3 md:grid-cols-2">
            {detail.alternatives.map((hit) => (
              <button key={hit.id} type="button" onClick={() => onOpen(hit.id)} className="flex items-center gap-3 rounded-2xl border border-line p-3 text-left">
                <Avatar name={hit.name} icon={hit.icon} />
                <span>
                  <span className="block text-[14px]">{hit.name}</span>
                  <span className="block text-[12px] text-muted">{hit.alternativeTo || hit.category}</span>
                </span>
              </button>
            ))}
          </div>
        </section>
      )}
    </div>
  )
}

function Avatar({ name, icon }: { name: string; icon: string }) {
  if (icon) return <img src={icon} alt="" loading="lazy" className="h-11 w-11 shrink-0 rounded-xl object-cover" />
  const letter = name.trim().charAt(0) || "?"
  return <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-soft text-[16px]">{letter}</span>
}

function Select({ label, value, options, onChange }: { label: string; value: string; options: { value: string; label: string }[]; onChange: (value: string) => void }) {
  return (
    <select aria-label={label} value={value} onChange={(event) => onChange(event.target.value)} className="h-10 rounded-[10px] border border-line bg-surface px-3 text-[14px]">
      {options.map((option) => (
        <option key={option.value || "empty"} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  )
}

function filterDemo(hits: CatalogHit[], collections: CatalogCollection[], query: Query) {
  const text = query.text.trim().toLowerCase()
  let rows = hits.filter((hit) => {
    if (text && !`${hit.name} ${hit.description} ${hit.alternativeTo}`.toLowerCase().includes(text)) return false
    if (query.category && hit.category !== query.category) return false
    if (query.openSource === "sim" && !hit.openSource) return false
    if (query.openSource === "nao" && hit.openSource) return false
    if (query.minStars && hit.stars < query.minStars) return false
    if ((query.recent || query.section === "trending") && Date.now() - hit.updatedMs > 180 * 86_400_000) return false
    if (query.installed === "sim" && !hit.installed) return false
    if (query.installed === "nao" && hit.installed) return false
    if (query.kind === "sim" && !hit.portable) return false
    if (query.kind === "nao" && !hit.installer) return false
    if (query.section === "alternatives" && !hit.alternativeTo) return false
    if (query.section === "favorites" && !hit.favorite) return false
    if (query.section === "collection") {
      const collection = collections.find((item) => item.id === query.collectionId)
      if (!collection?.apps.includes(hit.id)) return false
    }
    return true
  })
  const sort = query.section === "trending" || query.section === "stars" ? (query.sort === "relevancia" ? "estrelas" : query.sort) : query.sort
  rows = [...rows].sort((a, b) => {
    if (sort === "nome") return a.name.localeCompare(b.name, "pt")
    if (sort === "atualizacao") return b.updatedMs - a.updatedMs
    if (sort === "estrelas") return b.stars - a.stars
    return b.stars - a.stars
  })
  return rows
}
