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
  | "catalog"
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
  githubClientId: string
  notifyCatalogWatch: boolean
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

export type ManagerInfo = {
  id: string
  name: string
  installed: boolean
  version: string
  path: string
}

export type CatalogOffer = {
  source: string
  sourceId: string
  version: string
  size: number
  portable: boolean
  installer: boolean
}

export type CatalogHit = {
  id: string
  name: string
  description: string
  category: string
  license: string
  homepage: string
  repo: string
  wingetId: string
  stars: number
  updatedMs: number
  latestVersion: string
  size: number
  icon: string
  openSource: boolean
  portable: boolean
  installer: boolean
  alternativeTo: string
  sources: string[]
  offers: CatalogOffer[]
  installed: boolean
  favorite: boolean
  watched: boolean
}

export type CatalogAsset = { name: string; size: number; url: string; kind: string }

export type CatalogRelease = {
  version: string
  notes: string
  url: string
  publishedMs: number
  assets: CatalogAsset[]
}

export type CatalogDetail = CatalogHit & {
  readmeHtml: string
  images: string[]
  releases: CatalogRelease[]
  alternatives: CatalogHit[]
}

export type CatalogCollection = {
  id: string
  name: string
  preset: boolean
  apps: string[]
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
  "catalog",
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
