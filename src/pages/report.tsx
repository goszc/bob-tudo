import { useEffect, useState } from "react"
import { PageHeader } from "../components/page-header"
import { Button } from "../components/ui/button"
import { invoke, isTauri } from "../lib/api"
import { demoReport } from "../lib/desk-demo"

export function ReportPage() {
  const [report, setReport] = useState(demoReport)
  useEffect(() => {
    if (!isTauri()) return
    void invoke<typeof demoReport>("report_build").then(setReport).catch(() => {})
  }, [])

  function download(name: string, body: string, type: string) {
    const link = document.createElement("a")
    link.href = URL.createObjectURL(new Blob([body], { type }))
    link.download = name
    link.click()
  }

  return (
    <div>
      <PageHeader title="relatório do pc" sub="uma página com o que o bob tudo já sabe. html, pdf ou imagem — você escolhe quando exportar." />
      <div className="mb-4 flex flex-wrap gap-2">
        <Button onClick={() => void invoke<string>("report_html").then((html) => download("bob-tudo.html", html, "text/html")).catch(() => download("bob-tudo.html", documentHtml(report), "text/html"))}>html</Button>
        <Button variant="outline" onClick={() => void invoke<number[]>("report_pdf").then((bytes) => download("bob-tudo.pdf", String.fromCharCode(...bytes), "application/pdf")).catch(() => download("bob-tudo.txt", textReport(report), "text/plain"))}>pdf</Button>
        <Button variant="outline" onClick={() => drawImage(report)}>imagem</Button>
      </div>
      <div className="rounded-2xl border border-line bg-surface p-6">
        <p className="text-[12px] text-muted">{report.computer}</p>
        <h2 className="mt-1 text-[28px] tracking-[-0.04em]">{report.windows}</h2>
        <div className="mt-6 grid gap-4 sm:grid-cols-2">
          <Line label="processador" value={report.cpu} />
          <Line label="memória" value={report.ram} />
          <Line label="placa de vídeo" value={report.gpu} />
          <Line label="temperatura" value={report.temp} />
          <Line label="espaço" value={report.space} />
          <Line label="apps instalados" value={String(report.apps)} />
        </div>
        <div className="mt-6 text-[13px] text-muted">{report.disks.join(" · ")}</div>
        <div className="mt-1 text-[13px] text-muted">{report.health.join(" · ")}</div>
      </div>
    </div>
  )
}

function Line({ label, value }: { label: string; value: string }) {
  return <div><div className="text-[12px] text-muted">{label}</div><div className="text-[14px]">{value}</div></div>
}

function textReport(report: typeof demoReport) {
  return [report.computer, report.windows, report.cpu, report.ram, report.gpu, report.temp, report.space, `${report.apps} apps`, ...report.disks, ...report.health].join("\n")
}

function documentHtml(report: typeof demoReport) {
  return `<!doctype html><meta charset="utf-8"><title>bob tudo</title><body style="font-family:sans-serif"><h1>${report.computer}</h1><p>${report.windows}</p><p>${report.cpu}</p><p>${report.ram}</p></body>`
}

function drawImage(report: typeof demoReport) {
  const canvas = document.createElement("canvas")
  canvas.width = 1200
  canvas.height = 800
  const pen = canvas.getContext("2d")
  if (!pen) return
  pen.fillStyle = "#10141c"
  pen.fillRect(0, 0, 1200, 800)
  pen.fillStyle = "#e8eef8"
  pen.font = "28px sans-serif"
  pen.fillText(report.computer, 64, 96)
  pen.font = "42px sans-serif"
  pen.fillText(report.windows, 64, 160)
  pen.font = "24px sans-serif"
  ;[report.cpu, report.ram, report.gpu, report.temp, report.space, `${report.apps} apps`].forEach((line, index) => pen.fillText(line, 64, 240 + index * 48))
  const link = document.createElement("a")
  link.href = canvas.toDataURL("image/png")
  link.download = "bob-tudo.png"
  link.click()
}
