import { useState, type ReactNode } from "react"
import { Logo } from "../components/logo"
import { PageHeader } from "../components/page-header"
import { Button } from "../components/ui/button"
import { Switch } from "../components/ui/switch"
import { errorText, invoke, isTauri } from "../lib/api"
import type { ThemeMode } from "../lib/types"
import { setConfig, useApp } from "../store"
import { cn } from "../lib/cn"

const THEMES: { id: ThemeMode; label: string }[] = [
  { id: "light", label: "claro" },
  { id: "dark", label: "escuro" },
  { id: "glass-light", label: "vidro claro" },
  { id: "glass-dark", label: "vidro escuro" },
  { id: "system", label: "seguir o windows" },
]

export function SettingsPage() {
  const config = useApp().config
  const version = useApp().version
  const [checking, setChecking] = useState(false)
  const [message, setMessage] = useState("")

  async function check() {
    if (!isTauri()) {
      setMessage("no navegador a atualização não é baixada")
      return
    }
    setChecking(true)
    setMessage("")
    try {
      await invoke("check_now")
      setMessage("procura concluída")
    } catch (err) {
      setMessage(errorText(err))
    } finally {
      setChecking(false)
    }
  }

  return (
    <div>
      <PageHeader title="ajustes" sub="tema, bandeja, avisos e atualização. o bob tudo acompanha o windows e também deixa escolher." />
      <div className="mb-8 flex items-center gap-4">
        <Logo className="h-16 w-16" />
        <div>
          <p className="text-[16px]">bob tudo</p>
          <p className="text-[13px] text-muted">versão {version}</p>
        </div>
      </div>
      <h2 className="mb-3 text-[13px] text-muted">aparência</h2>
      <div className="mb-8 flex flex-wrap gap-2">
        {THEMES.map((theme) => (
          <button
            key={theme.id}
            type="button"
            onClick={() => setConfig({ theme: theme.id })}
            className={cn(
              "h-10 rounded-full border px-4 text-[14px]",
              config.theme === theme.id ? "border-fill bg-fill text-onfill" : "border-line bg-surface",
            )}
          >
            {theme.label}
          </button>
        ))}
      </div>
      <div className="flex max-w-xl flex-col gap-3">
        <Row label="fechar para a bandeja" detail="o x esconde a janela. sair de verdade fica no ícone ao lado do relógio.">
          <Switch checked={config.minimizeToTray} label="fechar para a bandeja" onCheckedChange={(minimizeToTray) => setConfig({ minimizeToTray })} />
        </Row>
        <Row label="iniciar com o windows" detail="abre minimizado na bandeja quando você entra na conta.">
          <Switch checked={config.startWithWindows} label="iniciar com o windows" onCheckedChange={(startWithWindows) => setConfig({ startWithWindows })} />
        </Row>
        <Row label="avisos" detail="liga ou desliga todos os avisos do windows.">
          <Switch checked={config.notifications} label="avisos" onCheckedChange={(notifications) => setConfig({ notifications })} />
        </Row>
        <Row label="disco quase cheio" detail="avisa quando o volume principal passa de 90%.">
          <Switch checked={config.notifyDiskFull} label="disco quase cheio" onCheckedChange={(notifyDiskFull) => setConfig({ notifyDiskFull })} />
        </Row>
        <Row label="atualizações de apps" detail="avisa quando o winget tem versão nova. não instala sozinho.">
          <Switch checked={config.notifyAppUpdates} label="atualizações de apps" onCheckedChange={(notifyAppUpdates) => setConfig({ notifyAppUpdates })} />
        </Row>
        <Row label="lixo acumulado" detail="avisa acima de 500 mb em temporários e caches.">
          <Switch checked={config.notifyJunk} label="lixo acumulado" onCheckedChange={(notifyJunk) => setConfig({ notifyJunk })} />
        </Row>
        <Row label="temperatura ou smart" detail="avisa disco quente ou com problema. não mexe no disco.">
          <Switch checked={config.notifySmart} label="temperatura ou smart" onCheckedChange={(notifySmart) => setConfig({ notifySmart })} />
        </Row>
        <Row label="tarefa terminou" detail="avisa quando uma limpeza, varredura ou instalação que você pediu acaba.">
          <Switch checked={config.notifyTaskDone} label="tarefa terminou" onCheckedChange={(notifyTaskDone) => setConfig({ notifyTaskDone })} />
        </Row>
        <Row label="atualizar o bob tudo" detail="confere o github ao abrir. não fica perguntando de meia em meia hora.">
          <Switch checked={config.autoUpdate} label="atualizar o bob tudo" onCheckedChange={(autoUpdate) => setConfig({ autoUpdate })} />
        </Row>
      </div>
      <div className="mt-8 flex items-center gap-3">
        <Button variant="outline" onClick={() => void check()} disabled={checking}>
          procurar atualização
        </Button>
        {message && <span className="text-[13px] text-muted">{message}</span>}
      </div>
    </div>
  )
}

function Row({ label, detail, children }: { label: string; detail: string; children: ReactNode }) {
  return (
    <div className="flex items-center gap-4 rounded-2xl border border-line px-4 py-3">
      <div className="min-w-0 flex-1">
        <p className="text-[14px]">{label}</p>
        <p className="text-[13px] text-muted">{detail}</p>
      </div>
      {children}
    </div>
  )
}
