import { useEffect, useState } from "react"
import { PageHeader } from "../components/page-header"
import { Segmented } from "../components/segmented"
import { Button } from "../components/ui/button"
import { invoke, isTauri } from "../lib/api"
import { demoDrivers, demoNet, demoPlans, type DriverInfo, type NetProc, type PowerPlan } from "../lib/desk-demo"
import { ask, toast, useApp } from "../store"

export function DriversPage() {
  const [rows, setRows] = useState<DriverInfo[]>(demoDrivers)
  useEffect(() => {
    if (!isTauri()) return
    void invoke<DriverInfo[]>("drivers_list").then((list) => list.length && setRows(list)).catch(() => {})
  }, [])
  return (
    <div>
      <PageHeader title="drivers" sub="versões e datas. um driver antigo vira uma sugestão — windows update, o site do fabricante ou um pacote do catálogo. eu não instalo sozinho." />
      <Button variant="outline" size="sm" className="mb-4" onClick={() => void invoke<DriverInfo[]>("drivers_refresh").then(setRows).catch(() => toast("a lista do windows aparece no app instalado"))}>atualizar lista</Button>
      <div className="space-y-3">
        {rows.map((row) => (
          <div key={row.device} className="rounded-2xl border border-line bg-surface p-4">
            <div className="flex items-start justify-between gap-3">
              <div>
                <h2 className="text-[15px]">{row.device}</h2>
                <p className="mt-1 text-[12px] text-muted">{row.className} · {row.version} · {row.date}</p>
              </div>
              {row.old && <span className="rounded-full bg-soft px-2 py-1 text-[12px] text-danger">antigo</span>}
            </div>
            <p className="mt-3 text-[14px] text-muted">{row.suggestion}</p>
          </div>
        ))}
      </div>
    </div>
  )
}

export function NetworkPage() {
  const [speed, setSpeed] = useState("240 mbps")
  const [ping, setPing] = useState("12 ms")
  const [preset, setPreset] = useState("cloudflare")
  const [procs, setProcs] = useState<NetProc[]>(demoNet)
  useEffect(() => {
    if (!isTauri()) return
    void invoke<NetProc[]>("net_processes").then((rows) => rows.length && setProcs(rows)).catch(() => {})
  }, [])
  return (
    <div>
      <PageHeader title="rede" sub="um teste curto na cloudflare, a latência, o dns e quem está com conexões abertas. trocar o dns pede permissão do windows." />
      <div className="grid gap-4 md:grid-cols-2">
        <div className="rounded-2xl border border-line bg-surface p-4">
          <div className="text-[13px] text-muted">velocidade</div>
          <p className="mt-2 text-[32px] tracking-[-0.04em]">{speed}</p>
          <Button className="mt-4" onClick={() => void invoke<{ mbps: number }>("net_speed").then((row) => setSpeed(`${Math.round(row.mbps)} mbps`)).catch(() => toast("teste no app instalado"))}>medir</Button>
        </div>
        <div className="rounded-2xl border border-line bg-surface p-4">
          <div className="text-[13px] text-muted">latência</div>
          <p className="mt-2 text-[32px] tracking-[-0.04em]">{ping}</p>
          <div className="mt-4 flex gap-2">
            <Button variant="outline" onClick={() => void invoke<number>("net_ping").then((ms) => setPing(`${Math.round(ms)} ms`)).catch(() => {})}>ping</Button>
            <Button variant="outline" onClick={() => void invoke("net_flush").then(() => toast("cache de dns limpo")).catch(() => toast("ipconfig no app instalado"))}>limpar dns</Button>
          </div>
        </div>
      </div>
      <div className="mt-4 rounded-2xl border border-line bg-surface p-4">
        <h2 className="text-[14px]">dns</h2>
        <div className="mt-3"><Segmented value={preset} onChange={setPreset} options={[{ value: "cloudflare", label: "cloudflare" }, { value: "google", label: "google" }, { value: "padrao", label: "padrão" }]} /></div>
        <Button className="mt-3" onClick={() => void invoke("dns_apply", { preset }).then(() => toast("o windows vai pedir permissão")).catch(() => toast("a troca de dns acontece no windows"))}>aplicar</Button>
      </div>
      <div className="mt-4 overflow-hidden rounded-2xl border border-line bg-surface">
        {procs.map((proc) => (
          <div key={proc.pid} className="flex items-center justify-between border-b border-line px-4 py-3 text-[14px] last:border-0">
            <span>{proc.name}</span>
            <span className="text-[12px] text-muted">{proc.connections} conexões</span>
          </div>
        ))}
      </div>
    </div>
  )
}

export function GamePage() {
  const config = useApp().config
  const [picked, setPicked] = useState<string[]>(config.gameApps)
  const [on, setOn] = useState(false)
  const names = ["discord", "spotify", "steam", "chrome"]
  return (
    <div>
      <PageHeader title="modo jogo" sub="um clique pausa as tarefas pesadas deste app, pede o plano de alto desempenho e, se você confirmar, fecha os programas da lista. ao sair, devolvo o plano." />
      <div className="rounded-2xl border border-line bg-surface p-4">
        <p className="text-[14px]">{on ? "modo jogo ligado · plano de alto desempenho" : "modo jogo desligado · plano equilibrado"}</p>
        <div className="mt-4 grid gap-2 sm:grid-cols-2">
          {names.map((name) => (
            <label key={name} className="flex items-center gap-2 rounded-[12px] border border-line px-3 py-2 text-[14px]">
              <input type="checkbox" checked={picked.includes(name)} onChange={() => setPicked((current) => current.includes(name) ? current.filter((item) => item !== name) : [...current, name])} />
              {name}
            </label>
          ))}
        </div>
        <div className="mt-4 flex gap-2">
          <Button onClick={() => ask("entrar no modo jogo?", "fecho os apps marcados e pauso as tarefas pesadas do bob tudo.", "entrar", () => { void invoke("game_enter", { close: picked }).catch(() => {}); setOn(true); toast("tarefas pesadas pausadas") })}>entrar</Button>
          <Button variant="outline" onClick={() => { void invoke("game_exit").catch(() => {}); setOn(false); toast("plano anterior de volta") }}>sair</Button>
        </div>
      </div>
    </div>
  )
}

export function PowerPage() {
  const [plans, setPlans] = useState<PowerPlan[]>(demoPlans)
  const [battery, setBattery] = useState({ designMwh: 48000, fullMwh: 44100, cycles: 120 })
  useEffect(() => {
    if (!isTauri()) return
    void invoke<PowerPlan[]>("power_list").then((rows) => rows.length && setPlans(rows)).catch(() => {})
    void invoke<typeof battery>("battery_report").then(setBattery).catch(() => {})
  }, [])
  const health = battery.designMwh ? Math.round((battery.fullMwh / battery.designMwh) * 100) : 0
  return (
    <div>
      <PageHeader title="energia" sub="troca o plano do windows. no notebook, leio o relatório da bateria: capacidade de projeto, carga completa e ciclos." />
      <div className="grid gap-4 md:grid-cols-[1.2fr_0.8fr]">
        <div className="overflow-hidden rounded-2xl border border-line bg-surface">
          {plans.map((plan) => (
            <button key={plan.guid} className="flex w-full items-center justify-between border-b border-line px-4 py-3 text-left text-[14px] last:border-0 hover:bg-soft" onClick={() => void invoke("power_set", { guid: plan.guid }).then(() => setPlans((rows) => rows.map((row) => ({ ...row, active: row.guid === plan.guid })))).catch(() => toast("o windows aplica o plano no app instalado"))}>
              <span>{plan.name}</span>
              {plan.active && <span className="text-[12px] text-muted">ativo</span>}
            </button>
          ))}
        </div>
        <div className="rounded-2xl border border-line bg-surface p-4">
          <h2 className="text-[14px]">bateria</h2>
          <p className="mt-2 text-[32px] tracking-[-0.04em]">{health}%</p>
          <p className="mt-2 text-[12px] text-muted">{battery.fullMwh} de {battery.designMwh} mwh · {battery.cycles} ciclos</p>
        </div>
      </div>
    </div>
  )
}
