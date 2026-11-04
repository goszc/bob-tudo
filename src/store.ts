import { useSyncExternalStore } from "react"
import { applyWindowTheme, invoke, isTauri, onBob } from "./lib/api"
import { isPage, isTheme, type Config, type Page, type ThemeMode } from "./lib/types"

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
}

const params = new URLSearchParams(location.search)
const storedTheme = localStorage.getItem("bob-tudo-theme")
const queryTheme = isTheme(params.get("theme")) ? params.get("theme") : null
const initialTheme = (queryTheme || (isTheme(storedTheme) ? storedTheme : "system")) as ThemeMode
const queryPage = params.get("page")

let state: State = {
  page: isPage(queryPage) ? queryPage : "disk",
  config: {
    theme: initialTheme,
    themeGlass: localStorage.getItem("bob-tudo-glass") === "1" || initialTheme === "glass-light" || initialTheme === "glass-dark",
    minimizeToTray: true,
    notifications: true,
    autoUpdate: true,
    updateUrl: "",
  },
  toast: null,
  confirm: null,
  updateOffer: null,
  version: "0.1.0",
}

const listeners = new Set<() => void>()
let toastTimer = 0
let booted = false

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
  patch({ page })
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
  void invoke<{ config: Config; version: string; updateReady: boolean }>("state").then((snapshot) => {
    patch({
      config: snapshot.config,
      version: snapshot.version,
      updateOffer: snapshot.updateReady ? snapshot.version : state.updateOffer,
    })
    applyTheme(snapshot.config.theme, snapshot.config.themeGlass)
  })
  void onBob((event) => {
    if (event.type === "toast") toast(event.message)
    if (event.type === "update") patch({ updateOffer: event.version })
  })
}

export async function applyUpdate() {
  if (!isTauri()) return
  await invoke("apply_update")
}
