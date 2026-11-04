import { useEffect, useState } from "react"
import { PageHeader } from "../components/page-header"
import { Segmented } from "../components/segmented"
import { invoke, isTauri } from "../lib/api"
import { demoGrew, demoSeries, type Grew, type Point } from "../lib/desk-demo"
import { bytes } from "../lib/format"

export function HistoryPage() {
  const [span, setSpan] = useState("semana")
  const [series, setSeries] = useState(demoSeries("semana"))
  const [grew, setGrew] = useState<Grew[]>(demoGrew)

  useEffect(() => {
    const local = demoSeries(span)
    if (!isTauri()) {
      setSeries(local)
      setGrew(demoGrew)
      return
    }
    void invoke<{ cpu: Point[]; ram: Point[]; temp: Point[]; folders: Grew[] }>("history_view", { span }).then((view) => {
      setSeries(view.cpu?.length ? { cpu: view.cpu, ram: view.ram, temp: view.temp } : local)
      if (view.folders?.length) setGrew(view.folders)
    }).catch(() => setSeries(local))
  }, [span])

  return (
    <div>
      <PageHeader title="histórico" sub="amostras leves enquanto o app está aberto. com o programa fechado, nada fica gravando." />
      <Segmented value={span} onChange={setSpan} options={[{ value: "dia", label: "dia" }, { value: "semana", label: "semana" }, { value: "mes", label: "mês" }]} />
      <div className="mt-4 grid gap-4 lg:grid-cols-3">
        <Chart title="cpu" unit="%" points={series.cpu} />
        <Chart title="memória" unit="%" points={series.ram} />
        <Chart title="temperatura" unit="°" points={series.temp} />
      </div>
      <div className="mt-4 rounded-2xl border border-line bg-surface p-4">
        <h2 className="text-[14px]">o que cresceu</h2>
        <div className="mt-3 space-y-2">
          {grew.map((row) => (
            <div key={row.key} className="flex items-center justify-between gap-3 text-[14px]">
              <span className="truncate">{row.key}</span>
              <span className="shrink-0 text-danger">+{bytes(Math.max(0, row.after - row.before))}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}

function Chart({ title, unit, points }: { title: string; unit: string; points: Point[] }) {
  const max = Math.max(...points.map((point) => point.value), 1)
  const width = 320
  const height = 120
  const path = points.map((point, index) => {
    const x = (index / Math.max(points.length - 1, 1)) * width
    const y = height - 12 - (point.value / max) * (height - 24)
    return `${index === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`
  }).join(" ")
  const last = points.at(-1)?.value ?? 0
  return (
    <div className="rounded-2xl border border-line bg-surface p-4">
      <div className="flex items-baseline justify-between">
        <h2 className="text-[14px]">{title}</h2>
        <span className="text-[12px] text-muted">{last.toFixed(0)}{unit}</span>
      </div>
      <svg viewBox={`0 0 ${width} ${height}`} className="mt-2 h-28 w-full text-ink">
        <path d={path} fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinejoin="round" />
      </svg>
    </div>
  )
}
