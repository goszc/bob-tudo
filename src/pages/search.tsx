import { useEffect, useState } from "react"
import { PageHeader } from "../components/page-header"
import { Segmented } from "../components/segmented"
import { Button } from "../components/ui/button"
import { Input } from "../components/ui/input"
import { invoke, isTauri } from "../lib/api"
import { demoFiles, filterFiles, type FileHit } from "../lib/desk-demo"
import { bytes } from "../lib/format"
import { toast } from "../store"

const kinds = ["", "documento", "video", "imagem", "programa", "pasta"]

export function SearchPage() {
  const [text, setText] = useState("")
  const [kind, setKind] = useState("")
  const [hits, setHits] = useState<FileHit[]>(demoFiles)

  useEffect(() => {
    const local = filterFiles(demoFiles, text, kind)
    if (!isTauri() || text.trim().length < 2) {
      setHits(local)
      return
    }
    void invoke<FileHit[]>("file_search", { query: { text, kind, limit: 80 } })
      .then((rows) => setHits(rows.length ? rows : local))
      .catch(() => setHits(local))
  }, [text, kind])

  return (
    <div>
      <PageHeader title="busca" sub="o nome aparece enquanto você digita. no windows a lista vem do índice do volume e o diário do ntfs mantém ela em dia." />
      <Input value={text} onChange={(event) => setText(event.target.value)} placeholder="notas, .mp4, downloads…" className="mb-4" />
      <Segmented value={kind} onChange={setKind} options={kinds.map((item) => ({ value: item, label: item || "tudo" }))} />
      <div className="mt-4 overflow-hidden rounded-2xl border border-line bg-surface">
        {hits.map((hit) => (
          <div key={hit.path} className="flex items-center gap-3 border-b border-line px-4 py-3 last:border-0">
            <div className="min-w-0 flex-1">
              <div className="truncate text-[14px]">{hit.name}</div>
              <div className="truncate text-[12px] text-muted">{hit.path}</div>
            </div>
            <span className="text-[12px] text-muted">{hit.dir ? "pasta" : bytes(hit.size)}</span>
            <Button variant="ghost" size="sm" onClick={() => void invoke("file_reveal", { path: hit.path }).then(() => toast("mostrando no explorer")).catch(() => toast(hit.path))}>mostrar</Button>
            <Button variant="ghost" size="sm" onClick={() => { void navigator.clipboard?.writeText(hit.path); void invoke("file_copy", { path: hit.path }).catch(() => {}); toast("caminho copiado") }}>copiar</Button>
          </div>
        ))}
        {hits.length === 0 && <p className="px-4 py-6 text-[14px] text-muted">nada com esse nome.</p>}
      </div>
    </div>
  )
}
