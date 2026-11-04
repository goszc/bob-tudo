import { useState } from "react"
import { PageHeader } from "../components/page-header"
import { Button } from "../components/ui/button"
import { errorText, invoke, isTauri } from "../lib/api"
import { demoHealth } from "../lib/demo"
import { bytes, hours, pct } from "../lib/format"
import type { DiskHealth } from "../lib/types"

export function HealthPage() {
  const [disks, setDisks] = useState<DiskHealth[]>(isTauri() ? [] : demoHealth)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState("")

  async function read(admin: boolean) {
    if (!isTauri()) {
      setDisks(demoHealth)
      return
    }
    setBusy(true)
    setError("")
    try {
      setDisks(await invoke<DiskHealth[]>("disk_health", { admin }))
    } catch (err) {
      setError(errorText(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <div>
      <PageHeader title="saúde dos discos" sub="temperatura, desgaste e smart. se o windows não contar tudo, o smartctl entra quando estiver instalado." />
      <div className="mb-5 flex gap-2">
        <Button onClick={() => void read(false)} disabled={busy}>
          ler discos
        </Button>
        <Button variant="outline" onClick={() => void read(true)} disabled={busy}>
          ler de novo como administrador
        </Button>
      </div>
      {error && <p className="mb-3 text-[13px] text-danger">{error}</p>}
      {!disks.length && !busy && <p className="text-[14px] text-muted">nenhuma leitura ainda. aperte ler discos.</p>}
      <div className="grid gap-3 md:grid-cols-2">
        {disks.map((disk) => (
          <article key={disk.name} className="rounded-2xl border border-line p-5">
            <div className="flex items-start justify-between gap-3">
              <div>
                <h2 className="text-[16px] font-medium tracking-tight">{disk.name}</h2>
                <p className="mt-1 text-[13px] text-muted">
                  {disk.media} · {bytes(disk.size)} · {disk.source}
                </p>
              </div>
              <span className={disk.alert ? "text-[13px] text-danger" : "text-[13px] text-muted"}>{disk.health}</span>
            </div>
            {disk.alert && <p className="mt-3 text-[13px] text-danger">{disk.alert}</p>}
            <dl className="mt-4 grid grid-cols-2 gap-3 text-[13px]">
              <Stat label="temperatura" value={disk.tempC == null ? "—" : `${disk.tempC} °c`} />
              <Stat label="desgaste" value={pct(disk.wear)} />
              <Stat label="horas ligado" value={hours(disk.hours)} />
              <Stat label="gravado" value={disk.tbwGb == null ? "—" : `${disk.tbwGb} gb`} />
              <Stat label="erros de leitura" value={disk.readErrors == null ? "—" : String(disk.readErrors)} />
              <Stat label="smart" value={disk.smartOk == null ? "—" : disk.smartOk ? "ok" : "falhou"} />
            </dl>
          </article>
        ))}
      </div>
    </div>
  )
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-muted">{label}</dt>
      <dd className="mt-0.5">{value}</dd>
    </div>
  )
}
