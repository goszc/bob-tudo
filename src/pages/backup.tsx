import { useEffect, useState } from "react"
import { PageHeader } from "../components/page-header"
import { Button } from "../components/ui/button"
import { invoke, isTauri } from "../lib/api"
import { demoBundle, type BundleApp } from "../lib/desk-demo"
import { toast } from "../store"

export function BackupPage() {
  const [apps, setApps] = useState<BundleApp[]>(demoBundle)

  useEffect(() => {
    if (!isTauri()) return
    void invoke<{ apps: BundleApp[] }>("bundle_preview").then((bundle) => {
      if (bundle.apps?.length) setApps(bundle.apps)
    }).catch(() => {})
  }, [])

  function save() {
    const body = JSON.stringify({ format: 1, apps, favorites: ["gimp"], collections: [{ name: "pc novo", apps: ["vlc", "gimp"] }] }, null, 2)
    if (!isTauri()) {
      const link = document.createElement("a")
      link.href = URL.createObjectURL(new Blob([body], { type: "application/json" }))
      link.download = "bob-tudo-apps.json"
      link.click()
      toast("backup baixado neste navegador")
      return
    }
    void invoke("bundle_save").then(() => toast("arquivo do backup escolhido")).catch(() => toast("não deu para salvar"))
  }

  return (
    <div>
      <PageHeader title="backup" sub="a lista de apps, os favoritos e as coleções. em outro pc, importar reinstala tudo numa tarefa só." />
      <div className="mb-4 flex gap-2">
        <Button onClick={save}>exportar</Button>
        <Button variant="outline" onClick={() => void invoke("bundle_import").then((count) => toast(`${count} apps na fila`)).catch(() => toast("escolha o arquivo no app instalado"))}>importar</Button>
      </div>
      <div className="overflow-hidden rounded-2xl border border-line bg-surface">
        {apps.map((app) => (
          <div key={`${app.source}:${app.id}`} className="flex items-center gap-3 border-b border-line px-4 py-3 text-[14px] last:border-0">
            <span className="w-24 text-[12px] text-muted">{app.source}</span>
            <span className="flex-1">{app.name}</span>
            <span className="text-[12px] text-muted">{app.version}</span>
          </div>
        ))}
      </div>
    </div>
  )
}
