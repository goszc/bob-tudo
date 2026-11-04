export type Page =
  | "home"
  | "disk"
  | "clean"
  | "dupes"
  | "health"
  | "startup"
  | "monitor"
  | "privacy"
  | "restore"
  | "install"
  | "oss"
  | "upgrades"
  | "uninstall"
  | "settings"

export type ThemeMode = "system" | "light" | "dark" | "glass-light" | "glass-dark"

export type Entry = {
  name: string
  path: string
  size: number
  kind: string
  dir: boolean
  synthetic: boolean
}

export type CleanCategory = {
  id: string
  title: string
  detail: string
  needsAdmin: boolean
  bytes: number
  files: number
  measured: boolean
  available: boolean
  checked: boolean
}

export type DupeFile = {
  path: string
  size: number
  modifiedMs: number
  keep: boolean
  distance?: number | null
}

export type DupeGroup = {
  id: string
  kind: string
  bytes: number
  files: DupeFile[]
}

export type DiskHealth = {
  name: string
  media: string
  health: string
  tempC: number | null
  hours: number | null
  wear: number | null
  tbwGb: number | null
  readErrors: number | null
  writeErrors: number | null
  smartOk: boolean | null
  size: number | null
  source: string
  alert: string | null
}

export type StartupItem = {
  id: string
  name: string
  command: string
  source: string
  enabled: boolean
  impact: string
  canToggle: boolean
  needsAdmin: boolean
}

export type PrivacyItem = {
  id: string
  title: string
  detail: string
  needsAdmin: boolean
  enabled: boolean
}

export type Change = {
  id: string
  at: string
  summary: string
  undone: boolean
}

export type RestorePoint = {
  sequence: number
  description: string
  when: string
}

export type ProcInfo = {
  pid: number
  name: string
  cpu: number
  ram: number
}

export type MonitorSample = {
  cpu: number
  ramUsed: number
  ramTotal: number
  diskUsed: number
  diskTotal: number
  diskBps: number
  gpu: number | null
  gpuName: string
  netDown: number
  netUp: number
  tempC: number | null
  processes: ProcInfo[]
}

export type WingetRow = {
  name: string
  id: string
  version: string
  available: string
  source: string
}

export type OssApp = {
  name: string
  description: string
  category: string
  repo: string
  wingetId: string
  alternativeTo: string
  stars: number
}

export type InstalledApp = {
  id: string
  name: string
  version: string
  publisher: string
  location: string
  uninstall: string
  quiet: string
  wingetId: string
  source: string
  size: number
}

export type Leftover = {
  path: string
  kind: string
  bytes: number
}

export type Config = {
  theme: ThemeMode
  themeGlass: boolean
  minimizeToTray: boolean
  notifications: boolean
  autoUpdate: boolean
  updateUrl: string
  startWithWindows: boolean
  notifyDiskFull: boolean
  notifyAppUpdates: boolean
  notifyJunk: boolean
  notifySmart: boolean
  notifyTaskDone: boolean
}

export type Job = {
  id: string
  title: string
  page: string
  progress: number
  detail: string
  state: "running" | "done" | "error"
}

export type Suggestion = {
  id: string
  title: string
  detail: string
  page: string
  tone: string
}

export type DiskSnap = {
  root: string
  total: number
  files: number
  errors: number
  children: Entry[]
  biggest: Entry[]
}

export const PAGES: Page[] = [
  "home",
  "disk",
  "clean",
  "dupes",
  "health",
  "startup",
  "monitor",
  "privacy",
  "restore",
  "install",
  "oss",
  "upgrades",
  "uninstall",
  "settings",
]

export function isPage(value: string | null): value is Page {
  return PAGES.includes(value as Page)
}

export function isTheme(value: string | null): value is ThemeMode {
  return value === "system" || value === "light" || value === "dark" || value === "glass-light" || value === "glass-dark"
}
