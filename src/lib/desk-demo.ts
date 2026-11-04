export type FileHit = {
  name: string
  path: string
  parent: string
  size: number
  modifiedMs: number
  dir: boolean
  kind: string
}

export type DriverInfo = {
  device: string
  className: string
  version: string
  date: string
  old: boolean
  suggestion: string
  channel: string
}

export type NetProc = { name: string; pid: number; connections: number }
export type PowerPlan = { guid: string; name: string; active: boolean }
export type Point = { ts: number; value: number }
export type Grew = { key: string; before: number; after: number }
export type BundleApp = { source: string; id: string; name: string; version: string }

const day = 86_400_000
const now = Date.now()

export const demoFiles: FileHit[] = [
  { name: "notas.txt", path: "C:\\Users\\ana\\Documents\\notas.txt", parent: "C:\\Users\\ana\\Documents", size: 80_000, modifiedMs: now - 2 * day, dir: false, kind: "documento" },
  { name: "aula-de-calculo.mp4", path: "C:\\Users\\ana\\Downloads\\aula-de-calculo.mp4", parent: "C:\\Users\\ana\\Downloads", size: 4_800_000_000, modifiedMs: now - 12 * day, dir: false, kind: "video" },
  { name: "relatorio-final.pdf", path: "C:\\Users\\ana\\Downloads\\relatorio-final.pdf", parent: "C:\\Users\\ana\\Downloads", size: 48_000_000, modifiedMs: now - 4 * day, dir: false, kind: "documento" },
  { name: "foto-viagem.jpg", path: "C:\\Users\\ana\\Pictures\\foto-viagem.jpg", parent: "C:\\Users\\ana\\Pictures", size: 8_400_000, modifiedMs: now - 30 * day, dir: false, kind: "imagem" },
  { name: "ensaio.mov", path: "C:\\Users\\ana\\Videos\\ensaio.mov", parent: "C:\\Users\\ana\\Videos", size: 12_400_000_000, modifiedMs: now - 20 * day, dir: false, kind: "video" },
  { name: "planilha.xlsx", path: "C:\\Users\\ana\\Documents\\planilha.xlsx", parent: "C:\\Users\\ana\\Documents", size: 6_400_000, modifiedMs: now - day, dir: false, kind: "documento" },
  { name: "bob-tudo.exe", path: "C:\\Users\\ana\\AppData\\Local\\bob tudo\\bob-tudo.exe", parent: "C:\\Users\\ana\\AppData\\Local\\bob tudo", size: 18_000_000, modifiedMs: now - 3 * day, dir: false, kind: "programa" },
  { name: "Downloads", path: "C:\\Users\\ana\\Downloads", parent: "C:\\Users\\ana", size: 11_000_000_000, modifiedMs: now - day, dir: true, kind: "pasta" },
]

export const demoDrivers: DriverInfo[] = [
  { device: "NVIDIA GeForce RTX 4060", className: "Display", version: "32.0.15.6094", date: "2023-11-02", old: true, suggestion: "o app da nvidia está no catálogo. não instalo sozinho.", channel: "winget" },
  { device: "Intel Wi-Fi 6E AX211", className: "Net", version: "23.40.0.4", date: "2025-06-18", old: false, suggestion: "vale olhar as atualizações opcionais do windows update.", channel: "windows" },
  { device: "Realtek High Definition Audio", className: "MEDIA", version: "6.0.9600.1", date: "2026-01-09", old: false, suggestion: "o fabricante publica a página do driver. eu só aponto.", channel: "vendor" },
]

export const demoNet: NetProc[] = [
  { name: "chrome", pid: 4412, connections: 38 },
  { name: "discord", pid: 2204, connections: 7 },
  { name: "bob-tudo", pid: 880, connections: 2 },
  { name: "spotify", pid: 3108, connections: 4 },
]

export const demoPlans: PowerPlan[] = [
  { guid: "381b4222-f694-41f0-9685-ff5bb260df2e", name: "equilibrado", active: true },
  { guid: "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c", name: "alto desempenho", active: false },
  { guid: "a1841308-3541-4fab-bc81-f71556f20b4a", name: "economia", active: false },
]

export const demoBundle: BundleApp[] = [
  { source: "winget", id: "VideoLAN.VLC", name: "vlc", version: "3.0.21" },
  { source: "winget", id: "GIMP.GIMP", name: "gimp", version: "2.10.38" },
  { source: "scoop", id: "bat", name: "bat", version: "0.24.0" },
  { source: "chocolatey", id: "paint.net", name: "paint.net", version: "5.1.4" },
]

function wave(count: number, base: number, amp: number, drift: number): Point[] {
  return Array.from({ length: count }, (_, index) => ({
    ts: now - (count - index) * day / 6,
    value: base + Math.sin(index / 2) * amp + index * drift,
  }))
}

export function demoSeries(span: string) {
  const count = span === "mes" ? 30 : span === "semana" ? 14 : 12
  return {
    cpu: wave(count, 18, 12, 0.1),
    ram: wave(count, 62, 8, 0.05),
    temp: wave(count, 42, 6, 0),
  }
}

export const demoGrew: Grew[] = [
  { key: "C:\\Users\\ana\\Videos", before: 40_000_000_000, after: 58_000_000_000 },
  { key: "C:\\Users\\ana\\Downloads", before: 8_000_000_000, after: 11_200_000_000 },
  { key: "C:\\", before: 210_000_000_000, after: 248_000_000_000 },
  { key: "D:\\", before: 90_000_000_000, after: 91_000_000_000 },
]

export const demoReport = {
  computer: "ana",
  windows: "windows 11 versão 24h2",
  cpu: "amd ryzen 7 · 18% agora",
  ram: "11 gb / 16 gb",
  gpu: "nvidia geforce rtx 4060",
  disks: ["C: · 248 gb em uso", "D: · 91 gb em uso"],
  health: ["samsung 980 · saudável", "seagate barracuda · atenção"],
  temp: "41 °c",
  space: "412 gb livres no volume principal",
  apps: 86,
}

export function filterFiles(files: FileHit[], text: string, kind: string) {
  const needle = text.trim().toLowerCase()
  return files.filter((file) => {
    if (needle && !file.name.toLowerCase().includes(needle) && !file.path.toLowerCase().includes(needle)) return false
    if (kind && file.kind !== kind) return false
    return true
  })
}

export const paletteActions: { id: string; label: string; hint: string; page: string }[] = [
  { id: "search", label: "buscar arquivos", hint: "nome em todo o disco", page: "search" },
  { id: "disk", label: "mapa do disco", hint: "o que ocupa espaço", page: "disk" },
  { id: "history", label: "histórico", hint: "gráficos e o que cresceu", page: "history" },
  { id: "clean", label: "limpeza", hint: "temporários e caches", page: "clean" },
  { id: "catalog", label: "catálogo", hint: "instalar apps", page: "catalog" },
  { id: "backup", label: "backup de apps", hint: "exportar e trazer de outro pc", page: "backup" },
  { id: "drivers", label: "drivers", hint: "só sugestão, sem instalar", page: "drivers" },
  { id: "network", label: "rede", hint: "velocidade, ping e dns", page: "network" },
  { id: "game", label: "modo jogo", hint: "pausa o bob tudo e sobe o desempenho", page: "game" },
  { id: "power", label: "energia", hint: "planos e bateria", page: "power" },
  { id: "report", label: "relatório do pc", hint: "uma página para compartilhar", page: "report" },
  { id: "settings", label: "ajustes", hint: "tema, agenda e github", page: "settings" },
]

export const trayTasks = [
  { title: "mapa do disco", progress: 0.62 },
  { title: "catálogo", progress: -1 },
]
