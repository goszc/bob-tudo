import type { ThemeMode } from "./types"

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window
}

export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core")
  return invoke<T>(command, args)
}

export type BobEvent =
  | { type: "scanProgress"; files: number; bytes: number; path: string }
  | { type: "dupeProgress"; phase: string; done: number; total: number }
  | { type: "jobLog"; line: string }
  | { type: "toast"; message: string }
  | { type: "update"; version: string }

export async function onBob(handler: (event: BobEvent) => void): Promise<() => void> {
  if (!isTauri()) return () => {}
  const { listen } = await import("@tauri-apps/api/event")
  return listen<BobEvent>("bob", (event) => handler(event.payload))
}

export async function applyWindowTheme(theme: ThemeMode, glassFamily: boolean) {
  if (!isTauri()) return
  const { getCurrentWindow } = await import("@tauri-apps/api/window")
  const win = getCurrentWindow()
  const dark =
    theme === "dark" ||
    theme === "glass-dark" ||
    (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches)
  const glass = theme === "glass-light" || theme === "glass-dark" || (theme === "system" && glassFamily)
  await win.setTheme(theme === "system" ? null : dark ? "dark" : "light")
  await invoke("set_window_material", { dark, glass })
}

export async function chooseFolder(): Promise<string | null> {
  if (!isTauri()) return null
  const { open } = await import("@tauri-apps/plugin-dialog")
  const picked = await open({ directory: true, multiple: false })
  return typeof picked === "string" ? picked : null
}

export function errorText(error: unknown): string {
  if (typeof error === "string") return error
  if (error instanceof Error) return error.message
  return String(error)
}
