import { PageHeader } from "../components/page-header"
import { Button } from "../components/ui/button"
import { bytes } from "../lib/format"
import { invoke, isTauri } from "../lib/api"
import { ask, go, toast, useApp } from "../store"
import type { Page } from "../lib/types"
import { isPage } from "../lib/types"

export function HomePage() {
  const app = useApp()
  const junk = app.clean.filter((item) => item.available && item.checked).reduce((sum, item) => sum + item.bytes, 0)
  const running = app.updating.length > 0

  function open(page: string) {
    if (isPage(page)) go(page as Page)
  }

  function cleanNow() {
    const ids = app.clean.filter((item) => item.checked && item.available).map((item) => item.id)
    if (!ids.length) {
      open("clean")
      return
    }
    ask("limpar o que foi medido", "só as categorias já marcadas. nada é apagado sem este passo.", "limpar", () => {
      if (!isTauri()) {
        toast("no navegador nada é apagado")
        return
      }
      void invoke("clean_run", { ids }).then(() => toast("limpeza pedida"))
    })
  }

  function upgradeNow() {
    ask("atualizar os apps", "o winget instala as versões novas. você confirma aqui.", "atualizar", () => {
      if (!isTauri()) {
        toast("no navegador o winget não atualiza")
        return
      }
      void invoke("winget_upgrade", { ids: [], all: true }).then(() => toast("atualização iniciada"))
    })
  }

  return (
    <div>
      <PageHeader
        title="sugestões"
        sub="o bob tudo olha o disco, o lixo, os apps, o smart e a inicialização ao abrir. ele só avisa. quem age é você."
      />
      {running && <p className="mb-4 text-[13px] text-muted">atualizando</p>}
      <div className="mb-6 grid gap-3 sm:grid-cols-3">
        <Stat label="mapa" value={app.disk ? bytes(app.disk.total) : "—"} />
        <Stat label="lixo marcado" value={junk ? bytes(junk) : "—"} />
        <Stat label="apps" value={app.upgrades.length ? String(app.upgrades.length) : "0"} />
      </div>
      {!app.suggestions.length && !running && (
        <p className="mb-4 text-[14px] text-muted">nada urgente nesta leitura. os números acima são a última vista.</p>
      )}
      <div className="flex flex-col gap-3">
        {app.suggestions.map((item) => (
          <article key={item.id} className="rounded-2xl border border-line px-4 py-4">
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div className="min-w-0">
                <h2 className="text-[16px] font-medium tracking-tight">{item.title}</h2>
                <p className="mt-1 text-[13px] text-muted">{item.detail}</p>
              </div>
              <div className="flex gap-2">
                <Button variant="outline" size="sm" onClick={() => open(item.page)}>
                  ver
                </Button>
                {item.id === "junk" && (
                  <Button size="sm" onClick={cleanNow}>
                    limpar
                  </Button>
                )}
                {item.id === "upgrades" && (
                  <Button size="sm" onClick={upgradeNow}>
                    atualizar
                  </Button>
                )}
              </div>
            </div>
          </article>
        ))}
      </div>
    </div>
  )
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-2xl border border-line px-4 py-3">
      <p className="text-[12px] text-muted">{label}</p>
      <p className="mt-1 text-[18px] tracking-tight">{value}</p>
    </div>
  )
}
