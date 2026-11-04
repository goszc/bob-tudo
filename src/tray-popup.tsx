import { useEffect, useState } from "react"
import { invoke, isTauri } from "./lib/api"
import { bytes } from "./lib/format"
import { trayTasks } from "./lib/desk-demo"
import type { MonitorSample } from "./lib/types"

const demo: MonitorSample = {
  cpu: 18,
  ramUsed: 11 * 1024 ** 3,
  ramTotal: 16 * 1024 ** 3,
  diskUsed: 248 * 1024 ** 3,
  diskTotal: 512 * 1024 ** 3,
  diskBps: 0,
  gpu: 22,
  gpuName: "rtx 4060",
  netDown: 1_200_000,
  netUp: 180_000,
  tempC: 48,
  processes: [
    { pid: 4412, name: "chrome", cpu: 9, ram: 1_800_000_000 },
    { pid: 2204, name: "discord", cpu: 3, ram: 420_000_000 },
    { pid: 880, name: "bob-tudo", cpu: 1, ram: 180_000_000 },
  ],
}

export function TrayPopup() {
  const [sample, setSample] = useState<MonitorSample>(demo)
  const [jobs, setJobs] = useState(trayTasks)

  useEffect(() => {
    document.documentElement.classList.add("glass")
    if (!isTauri()) return
    void invoke("monitor_tray", { on: true }).catch(() => {})
    const tick = () => {
      void invoke<MonitorSample>("monitor_sample").then(setSample).catch(() => {})
      void invoke<{ title: string; progress: number; state: string }[]>("jobs_list").then((rows) => {
        const running = rows.filter((row) => row.state === "running")
        if (running.length) setJobs(running.map((row) => ({ title: row.title, progress: row.progress })))
      }).catch(() => {})
    }
    tick()
    const timer = window.setInterval(tick, 2000)
    return () => {
      window.clearInterval(timer)
      void invoke("monitor_tray", { on: false }).catch(() => {})
    }
  }, [])

  const ram = sample.ramTotal ? Math.round((sample.ramUsed / sample.ramTotal) * 100) : 0
  const diskPct = sample.diskTotal ? Math.round((sample.diskUsed / sample.diskTotal) * 100) : 0

  return (
    <div className="h-screen w-screen bg-transparent p-2">
      <div className="flex h-full flex-col rounded-3xl border border-line bg-surface/80 p-4 text-ink shadow-2xl backdrop-blur-xl">
        <div className="text-[14px]">bob tudo</div>
        <div className="mt-3 grid grid-cols-2 gap-2">
          <Meter label="cpu" value={`${sample.cpu.toFixed(0)}%`} width={sample.cpu} />
          <Meter label="memória" value={`${ram}%`} width={ram} />
          <Meter label="gpu" value={sample.gpu == null ? "—" : `${sample.gpu.toFixed(0)}%`} width={sample.gpu ?? 0} />
          <Meter label="temp" value={sample.tempC == null ? "—" : `${sample.tempC.toFixed(0)}°`} width={sample.tempC ?? 0} />
          <Meter label="disco" value={`${diskPct}%`} width={diskPct} />
          <Meter label="rede" value={bytes(sample.netDown)} width={30} />
        </div>
        <div className="mt-4 text-[11px] text-muted">tarefas</div>
        <div className="mt-2 space-y-2">
          {jobs.map((job) => (
            <div key={job.title}>
              <div className="flex justify-between text-[12px]"><span>{job.title}</span><span>{job.progress < 0 ? "em curso" : `${Math.round(job.progress * 100)}%`}</span></div>
              <div className="mt-1 h-1 overflow-hidden rounded-full bg-line"><div className="h-full bg-fill" style={{ width: `${job.progress < 0 ? 35 : job.progress * 100}%` }} /></div>
            </div>
          ))}
        </div>
        <div className="mt-4 text-[11px] text-muted">em execução</div>
        <div className="mt-1 min-h-0 flex-1 space-y-1 overflow-hidden">
          {sample.processes.slice(0, 4).map((proc) => (
            <div key={proc.pid} className="flex justify-between text-[12px]">
              <span>{proc.name}</span>
              <span className="text-muted">{proc.cpu.toFixed(0)}%</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}

function Meter({ label, value, width }: { label: string; value: string; width: number }) {
  return (
    <div className="rounded-2xl bg-soft/80 px-3 py-2">
      <div className="flex justify-between text-[12px] text-muted"><span>{label}</span><span className="text-ink">{value}</span></div>
      <div className="mt-2 h-1 overflow-hidden rounded-full bg-line"><div className="h-full bg-fill" style={{ width: `${Math.max(4, Math.min(100, width))}%` }} /></div>
    </div>
  )
}
