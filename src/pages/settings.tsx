import { useEffect, useState, type ReactNode } from "react"
import { Logo } from "../components/logo"
import { PageHeader } from "../components/page-header"
import { Button } from "../components/ui/button"
import { Input } from "../components/ui/input"
import { Switch } from "../components/ui/switch"
import { errorText, invoke, isTauri } from "../lib/api"
import type { ManagerInfo, ThemeMode } from "../lib/types"
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
      <section id="agenda" className="mb-8 max-w-xl">
        <h2 className="mb-3 text-[13px] text-muted">agenda</h2>
        <div className="flex flex-col gap-3">
          <Row label="avisos agendados" detail="opcional. ao abrir, avisa se a limpeza ou a saúde do disco passou do prazo. não acorda o pc e não age sozinho.">
            <Switch checked={config.scheduleEnabled} label="avisos agendados" onCheckedChange={(scheduleEnabled) => setConfig({ scheduleEnabled })} />
          </Row>
          <Row label="token do github" detail="alternativa ao login. fica no cofre do windows e nunca volta para esta tela.">
            <TokenField />
          </Row>
        </div>
      </section>
      <Managers />
      <Github />
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
        <Row label="versão nova no catálogo" detail="avisa, ao abrir, quando um app que você marcou lançou versão.">
          <Switch checked={config.notifyCatalogWatch} label="versão nova no catálogo" onCheckedChange={(notifyCatalogWatch) => setConfig({ notifyCatalogWatch })} />
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

const MANAGER_DETAIL: Record<string, string> = {
  winget: "app installer oficial. se não entrar, abre a loja da microsoft.",
  scoop: "instalação do seu usuário, sem administrador.",
  chocolatey: "pede permissão de administrador.",
}

function Managers() {
  const managers = useApp().managers
  const refreshing = useApp().updating.includes("managers")
  const [note, setNote] = useState("")

  async function run(id: string, update: boolean) {
    if (!isTauri()) {
      setNote("no navegador o gerenciador não é instalado")
      return
    }
    setNote("")
    try {
      await invoke(update ? "manager_update" : "manager_install", { id })
      setNote(id === "chocolatey" && !update ? "o windows vai pedir permissão" : "começou")
    } catch (err) {
      setNote(errorText(err))
    }
  }

  return (
    <section id="gerenciadores" className="mb-8 max-w-xl">
      <h2 className="mb-3 text-[13px] text-muted">gerenciadores de pacotes</h2>
      <div className="flex flex-col gap-3">
        {managers.map((manager) => (
          <ManagerRow key={manager.id} manager={manager} onInstall={() => void run(manager.id, false)} onUpdate={() => void run(manager.id, true)} />
        ))}
      </div>
      <p className="mt-2 text-[12px] text-muted">{refreshing ? "conferindo de novo" : "conferido ao abrir e guardado para a próxima vez"}{note ? ` · ${note}` : ""}</p>
    </section>
  )
}

function ManagerRow({ manager, onInstall, onUpdate }: { manager: ManagerInfo; onInstall: () => void; onUpdate: () => void }) {
  return (
    <div className="flex items-center gap-4 rounded-2xl border border-line px-4 py-3">
      <div className="min-w-0 flex-1">
        <p className="text-[14px]">{manager.name}</p>
        <p className="truncate text-[13px] text-muted">
          {manager.installed ? `${manager.version || "instalado"} · ${manager.path}` : "não instalado"}
        </p>
        <p className="text-[12px] text-muted">{MANAGER_DETAIL[manager.id]}</p>
      </div>
      {manager.installed ? (
        <Button size="sm" variant="outline" onClick={onUpdate}>
          atualizar
        </Button>
      ) : (
        <Button size="sm" onClick={onInstall}>
          instalar
        </Button>
      )}
    </div>
  )
}

function Github() {
  const config = useApp().config
  const [clientId, setClientId] = useState(config.githubClientId)
  const [login, setLogin] = useState("")
  const [code, setCode] = useState("")
  const [uri, setUri] = useState("")
  const [note, setNote] = useState("")

  useEffect(() => {
    if (!isTauri()) return
    void invoke<{ connected: boolean; login: string }>("github_status")
      .then((state) => setLogin(state.connected ? state.login || "conectado" : ""))
      .catch(() => {})
  }, [])

  async function begin() {
    setConfig({ githubClientId: clientId.trim() })
    if (!isTauri()) {
      setNote("no navegador o login não conecta")
      return
    }
    try {
      const started = await invoke<{ userCode: string; verificationUri: string }>("github_login_begin")
      setCode(started.userCode)
      setUri(started.verificationUri)
      setNote("autorize no github e volte")
    } catch (err) {
      setNote(errorText(err))
    }
  }

  async function poll() {
    if (!isTauri()) return
    try {
      const state = await invoke<{ connected: boolean; login: string }>("github_login_poll")
      setLogin(state.connected ? state.login || "conectado" : "")
      setNote(state.connected ? "conectado" : "ainda esperando")
      if (state.connected) {
        setCode("")
        setUri("")
      }
    } catch (err) {
      setNote(errorText(err))
    }
  }

  async function logout() {
    if (!isTauri()) return
    await invoke("github_logout")
    setLogin("")
    setNote("saiu")
  }

  return (
    <section className="mb-8 max-w-xl">
      <h2 className="mb-3 text-[13px] text-muted">github</h2>
      <div className="rounded-2xl border border-line px-4 py-3">
        <p className="text-[14px]">limite da api</p>
        <p className="text-[13px] text-muted">sem login o catálogo usa cache. o client id é de um app oauth seu, no fluxo de dispositivo.</p>
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <Input className="max-w-[240px]" placeholder="client id" value={clientId} onChange={(event) => setClientId(event.target.value)} onBlur={() => setConfig({ githubClientId: clientId.trim() })} />
          {login ? (
            <Button size="sm" variant="outline" onClick={() => void logout()}>
              sair · {login}
            </Button>
          ) : (
            <Button size="sm" onClick={() => void begin()}>
              entrar
            </Button>
          )}
          {code && (
            <Button size="sm" variant="outline" onClick={() => void poll()}>
              já autorizei
            </Button>
          )}
        </div>
        {code && (
          <p className="mt-2 text-[13px]">
            código {code}
            {uri ? (
              <>
                {" "}
                · <a className="underline decoration-line underline-offset-4" href={uri} target="_blank" rel="noreferrer">{uri}</a>
              </>
            ) : null}
          </p>
        )}
        {note && <p className="mt-2 text-[12px] text-muted">{note}</p>}
      </div>
    </section>
  )
}

function TokenField() {
  const [token, setToken] = useState("")
  const [note, setNote] = useState("")
  return (
    <form
      className="flex items-center gap-2"
      onSubmit={(event) => {
        event.preventDefault()
        if (!isTauri()) {
          setNote("no app instalado")
          setToken("")
          return
        }
        void invoke("github_set_token", { token }).then(() => {
          setToken("")
          setNote(token.trim() ? "guardado" : "removido")
        }).catch((err) => setNote(errorText(err)))
      }}
    >
      <Input type="password" className="w-[180px]" placeholder="ghp_…" value={token} onChange={(event) => setToken(event.target.value)} autoComplete="off" />
      <Button size="sm" type="submit">guardar</Button>
      {note && <span className="text-[12px] text-muted">{note}</span>}
    </form>
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
