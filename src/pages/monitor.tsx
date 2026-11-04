import { useEffect, useState } from "react"
import { PageHeader } from "../components/page-header"
import { errorText, invoke, isTauri } from "../lib/api"
import { demoMonitor } from "../lib/demo"
import { bytes, pct, rate } from "../lib/format"
import type { MonitorSample } from "../lib/types"
import { ask, toast } from "../store"

export function MonitorPage() {
  const [sample, setSample] = useState<MonitorSample | null>(isTauri() ? null : demoMonitor)
  const [error, setError] = useState("")

  useEffect(() => {
    if (!isTauri()) return
    let alive = true
    const tick = async () => {
      try {
        const next = await invoke<MonitorSample>("monitor_sample")
        if (alive) setSample(next)
      } catch (err) {
        if (alive) setError(errorText(err))
      }
    }
    void tick()
    const timer = window.setInterval(() => void tick(), 2000)
    return () => {
      alive = false
      window.clearInterval(timer)
    }
  }, [])

  function kill(pid: number, name: string) {
    ask("encerrar processo", `${name} (${pid}) fecha agora.`, "encerrar", () => {
      void (async () => {
        if (!isTauri()) {
          toast("no navegador nenhum processo fecha")
          return
        }
        await invoke("kill_process", { pid })
        toast(`${name} encerrado`)
      })()
    })
  }

  if (!sample) {
    return (
      <div>
        <PageHeader title="monitor" sub="cpu, memória, disco, placa de vídeo, rede e temperatura, atualizando sozinho." />
        <p className="text-[14px] text-muted">{error || "esperando a primeira leitura…"}</p>
      </div>
    )
  }

  const ram = sample.ramTotal ? sample.ramUsed / sample.ramTotal : 0
  const disk = sample.diskTotal ? sample.diskUsed / sample.diskTotal : 0

  return (
    <div>
      <PageHeader title="monitor" sub="cpu, memória, disco, placa de vídeo, rede e temperatura, atualizando sozinho." />
      {error && <p className="mb-3 text-[13px] text-danger">{error}</p>}
      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        <Meter label="cpu" value={pct(sample.cpu)} ratio={sample.cpu / 100} />
        <Meter label="memória" value={`${bytes(sample.ramUsed)} de ${bytes(sample.ramTotal)}`} ratio={ram} />
        <Meter label="disco" value={`${bytes(sample.diskUsed)} de ${bytes(sample.diskTotal)} · ${rate(sample.diskBps)}`} ratio={disk} />
        <Meter label={sample.gpuName || "placa de vídeo"} value={sample.gpu == null ? "—" : pct(sample.gpu)} ratio={(sample.gpu ?? 0) / 100} />
        <Meter label="rede" value={`↓ ${rate(sample.netDown)} · ↑ ${rate(sample.netUp)}`} ratio={0} />
        <Meter label="temperatura" value={sample.tempC == null ? "—" : `${Math.round(sample.tempC)} °c`} ratio={sample.tempC == null ? 0 : Math.min(1, sample.tempC / 100)} />
      </div>
      <h2 className="mb-3 mt-8 text-[15px]">processos</h2>
      <div className="overflow-hidden rounded-2xl border border-line">
        {sample.processes.map((proc) => (
          <div key={proc.pid} className="flex items-center gap-3 border-b border-line px-4 py-3 text-[13px] last:border-b-0">
            <span className="w-28 truncate">{proc.name}</span>
            <span className="w-16 text-muted">{pct(proc.cpu)}</span>
            <span className="w-20 text-muted">{bytes(proc.ram)}</span>
            <span className="flex-1 text-muted">{proc.pid}</span>
            <button type="button" className="text-danger" onClick={() => kill(proc.pid, proc.name)}>
              encerrar
            </button>
          </div>
        ))}
      </div>
    </div>
  )
}

function Meter({ label, value, ratio }: { label: string; value: string; ratio: number }) {
  return (
    <div className="rounded-2xl border border-line p-4">
      <p className="text-[12px] text-muted">{label}</p>
      <p className="mt-1 text-[18px] tracking-tight">{value}</p>
      <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-soft">
        <div className="h-full rounded-full bg-fill" style={{ width: `${Math.max(0, Math.min(1, ratio)) * 100}%` }} />
      </div>
    </div>
  )
}
