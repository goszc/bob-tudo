import { useSyncExternalStore } from "react"
import { applyWindowTheme, invoke, isTauri, onBob } from "./lib/api"
import { demoManagers } from "./lib/catalog-demo"
import { demoBiggest, demoChildren, demoClean, demoHealth, demoRoot, demoStartup, demoUpgrades } from "./lib/demo"
import { isPage, isTheme, type CleanCategory, type Config, type DiskHealth, type DiskSnap, type Job, type ManagerInfo, type Page, type StartupItem, type Suggestion, type ThemeMode, type WingetRow } from "./lib/types"

type Confirm = {
  title: string
  body: string
  confirm: string
  cancel: string
  run: () => void
}

type State = {
  page: Page
  config: Config
  toast: string | null
  confirm: Confirm | null
  updateOffer: string | null
  version: string
  jobs: Job[]
  suggestions: Suggestion[]
  disk: DiskSnap | null
  clean: CleanCategory[]
  health: DiskHealth[]
  startup: StartupItem[]
  upgrades: WingetRow[]
  updating: string[]
  managers: ManagerInfo[]
}

const params = new URLSearchParams(location.search)
const storedTheme = localStorage.getItem("bob-tudo-theme")
const queryTheme = isTheme(params.get("theme")) ? params.get("theme") : null
const initialTheme = (queryTheme || (isTheme(storedTheme) ? storedTheme : "system")) as ThemeMode
const queryPage = params.get("page")
const browser = typeof window !== "undefined" && !isTauri()

const browserDisk: DiskSnap | null = browser
  ? { root: demoRoot.path, total: demoRoot.size, files: 12, errors: 0, children: demoChildren(demoRoot.path), biggest: demoBiggest(false) }
  : null

let state: State = {
  page: asPage(queryPage),
  config: {
    theme: initialTheme,
    themeGlass: localStorage.getItem("bob-tudo-glass") === "1" || initialTheme === "glass-light" || initialTheme === "glass-dark",
    minimizeToTray: true,
    notifications: true,
    autoUpdate: true,
    updateUrl: "",
    startWithWindows: true,
    notifyDiskFull: true,
    notifyAppUpdates: true,
    notifyJunk: true,
    notifySmart: true,
    notifyTaskDone: true,
    githubClientId: "",
    notifyCatalogWatch: true,
  },
  toast: null,
  confirm: null,
  updateOffer: null,
  version: "0.3.0",
  jobs: browser
    ? [{ id: "mapa", title: "mapa do disco", page: "disk", progress: 0.62, detail: "c:\\users", state: "running" }]
    : [],
  suggestions: browser
    ? [
        { id: "disk-full", title: "disco quase cheio", detail: "resta cerca de 18 gb livres no volume principal", page: "disk", tone: "warn" },
        { id: "junk", title: "lixo acumulado", detail: "6 gb em temporários e caches, se você quiser limpar", page: "clean", tone: "warn" },
        { id: "upgrades", title: "atualizações de apps", detail: "2 aplicativo(s) com versão nova no winget", page: "upgrades", tone: "info" },
        { id: "smart", title: "disco com problema", detail: "o seagate barracuda não está saudável", page: "health", tone: "warn" },
      ]
    : [],
  disk: browserDisk,
  clean: browser ? demoClean : [],
  health: browser ? demoHealth : [],
  startup: browser ? demoStartup : [],
  upgrades: browser ? demoUpgrades : [],
  updating: browser ? ["disk"] : [],
  managers: browser ? demoManagers : [],
}

const listeners = new Set<() => void>()
let toastTimer = 0
let booted = false

function asPage(value: string | null): Page {
  if (value === "install" || value === "oss") return "catalog"
  return isPage(value) ? value : "home"
}

function emit() {
  listeners.forEach((listener) => listener())
}

function patch(next: Partial<State>) {
  state = { ...state, ...next }
  emit()
}

export function getState() {
  return state
}

export function useApp() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    getState,
    getState,
  )
}

export function resolveTheme(theme: ThemeMode, glassFamily: boolean) {
  const systemDark = window.matchMedia("(prefers-color-scheme: dark)").matches
  const dark = theme === "dark" || theme === "glass-dark" || (theme === "system" && systemDark)
  const glass = theme === "glass-light" || theme === "glass-dark" || (theme === "system" && glassFamily)
  return { dark, glass }
}

export function applyTheme(theme: ThemeMode, glassFamily = getState().config.themeGlass) {
  const { dark, glass } = resolveTheme(theme, glassFamily)
  document.documentElement.classList.toggle("dark", dark)
  document.documentElement.classList.toggle("glass", glass)
  document.documentElement.style.colorScheme = dark ? "dark" : "light"
  localStorage.setItem("bob-tudo-theme", theme)
  localStorage.setItem("bob-tudo-glass", glassFamily ? "1" : "0")
  void applyWindowTheme(theme, glassFamily)
}

export function go(page: Page) {
  patch({ page: page === "install" || page === "oss" ? "catalog" : page })
}

export function toast(message: string) {
  patch({ toast: message })
  window.clearTimeout(toastTimer)
  toastTimer = window.setTimeout(() => patch({ toast: null }), 2600)
}

export function ask(title: string, body: string, confirm: string, run: () => void) {
  patch({ confirm: { title, body, confirm, cancel: "cancelar", run } })
}

export function closeConfirm() {
  patch({ confirm: null })
}

export function setConfig(next: Partial<Config>) {
  let themeGlass = next.themeGlass ?? state.config.themeGlass
  if (next.theme === "light" || next.theme === "dark") themeGlass = false
  if (next.theme === "glass-light" || next.theme === "glass-dark") themeGlass = true
  const config = { ...state.config, ...next, themeGlass }
  patch({ config })
  if (next.theme || next.themeGlass !== undefined) applyTheme(config.theme, config.themeGlass)
  if (isTauri()) void invoke("set_config", { config })
}

export function initApp() {
  if (booted) return
  booted = true
  applyTheme(state.config.theme, state.config.themeGlass)
  const media = window.matchMedia("(prefers-color-scheme: dark)")
  media.addEventListener("change", () => {
    if (getState().config.theme === "system") applyTheme("system")
  })
  if (!isTauri()) return
  void invoke<{
    config: Config
    version: string
    updateReady: boolean
    jobs: Job[]
    disk: DiskSnap | null
    clean: CleanCategory[]
    health: DiskHealth[]
    startup: StartupItem[]
    upgrades: WingetRow[]
    suggestions: Suggestion[]
    updating: string[]
    managers: ManagerInfo[]
  }>("state").then((snapshot) => {
    patch({
      config: { ...state.config, ...snapshot.config },
      version: snapshot.version,
      updateOffer: snapshot.updateReady ? snapshot.version : state.updateOffer,
      jobs: snapshot.jobs ?? [],
      disk: snapshot.disk,
      clean: snapshot.clean ?? [],
      health: snapshot.health ?? [],
      startup: snapshot.startup ?? [],
      upgrades: snapshot.upgrades ?? [],
      suggestions: snapshot.suggestions ?? [],
      updating: snapshot.updating ?? [],
      managers: snapshot.managers ?? [],
    })
    applyTheme(snapshot.config.theme, snapshot.config.themeGlass)
  })
  void onBob((event) => {
    if (event.type === "toast") toast(event.message)
    if (event.type === "update") patch({ updateOffer: event.version })
    if (event.type === "jobs") patch({ jobs: event.jobs })
    if (event.type === "navigate" && isPage(event.page)) go(event.page)
    if (event.type === "disk") patch({ disk: event.snapshot })
    if (event.type === "clean") patch({ clean: event.items })
    if (event.type === "health") patch({ health: event.disks })
    if (event.type === "startup") patch({ startup: event.items })
    if (event.type === "upgrades") patch({ upgrades: event.rows })
    if (event.type === "suggestions") patch({ suggestions: event.items })
    if (event.type === "managers") patch({ managers: event.managers })
    if (event.type === "updating") {
      const updating = event.on ? Array.from(new Set([...state.updating, event.area])) : state.updating.filter((area) => area !== event.area)
      patch({ updating })
    }
  })
}

export async function applyUpdate() {
  if (!isTauri()) return
  await invoke("apply_update")
}
