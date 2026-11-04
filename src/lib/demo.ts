import type { CleanCategory, DiskHealth, DupeGroup, Entry, InstalledApp, Leftover, MonitorSample, OssApp, PrivacyItem, RestorePoint, StartupItem, WingetRow, Change } from "./types"

type Node = Entry & { children?: Node[] }

function file(name: string, path: string, size: number, kind: string): Node {
  return { name, path, size, kind, dir: false, synthetic: false }
}

function folder(name: string, path: string, kind: string, children: Node[]): Node {
  const size = children.reduce((sum, child) => sum + child.size, 0)
  return { name, path, size, kind, dir: true, synthetic: false, children }
}

export const demoRoot = folder("ana", "C:\\Users\\ana", "pasta", [
  folder("Downloads", "C:\\Users\\ana\\Downloads", "arquivo", [
    file("aula-de-calculo.mp4", "C:\\Users\\ana\\Downloads\\aula-de-calculo.mp4", 4_800_000_000, "video"),
    file("aula-de-calculo (1).mp4", "C:\\Users\\ana\\Downloads\\aula-de-calculo (1).mp4", 4_800_000_000, "video"),
    file("dataset.zip", "C:\\Users\\ana\\Downloads\\dataset.zip", 6_200_000_000, "arquivo"),
    file("relatorio-final.pdf", "C:\\Users\\ana\\Downloads\\relatorio-final.pdf", 48_000_000, "documento"),
    file("foto-viagem.jpg", "C:\\Users\\ana\\Downloads\\foto-viagem.jpg", 8_400_000, "imagem"),
  ]),
  folder("Videos", "C:\\Users\\ana\\Videos", "video", [
    file("ensaio.mov", "C:\\Users\\ana\\Videos\\ensaio.mov", 12_400_000_000, "video"),
    file("ensaio-copia.mov", "C:\\Users\\ana\\Videos\\ensaio-copia.mov", 12_400_000_000, "video"),
    file("chamada.mkv", "C:\\Users\\ana\\Videos\\chamada.mkv", 2_100_000_000, "video"),
  ]),
  folder("AppData", "C:\\Users\\ana\\AppData", "codigo", [
    folder("Local", "C:\\Users\\ana\\AppData\\Local", "codigo", [
      folder("Google", "C:\\Users\\ana\\AppData\\Local\\Google", "outro", [
        file("cache", "C:\\Users\\ana\\AppData\\Local\\Google\\Chrome\\User Data\\Default\\Cache", 3_400_000_000, "outro"),
      ]),
      file("node_modules", "C:\\Users\\ana\\AppData\\Local\\npm-cache", 5_600_000_000, "codigo"),
    ]),
  ]),
  folder("Documents", "C:\\Users\\ana\\Documents", "documento", [
    file("tese.docx", "C:\\Users\\ana\\Documents\\tese.docx", 22_000_000, "documento"),
    file("notas.txt", "C:\\Users\\ana\\Documents\\notas.txt", 80_000, "documento"),
    file("planilha.xlsx", "C:\\Users\\ana\\Documents\\planilha.xlsx", 6_400_000, "documento"),
  ]),
  folder("Pictures", "C:\\Users\\ana\\Pictures", "imagem", [
    file("praia.jpg", "C:\\Users\\ana\\Pictures\\praia.jpg", 9_200_000, "imagem"),
    file("praia-editada.jpg", "C:\\Users\\ana\\Pictures\\praia-editada.jpg", 8_800_000, "imagem"),
    file("scan.png", "C:\\Users\\ana\\Pictures\\scan.png", 14_000_000, "imagem"),
  ]),
  folder("Desktop", "C:\\Users\\ana\\Desktop", "programa", [
    file("atalho.lnk", "C:\\Users\\ana\\Desktop\\atalho.lnk", 2_000, "programa"),
    file("rascunho.txt", "C:\\Users\\ana\\Desktop\\rascunho.txt", 12_000, "documento"),
  ]),
])

function find(node: Node, path: string): Node | undefined {
  if (node.path === path) return node
  for (const child of node.children ?? []) {
    const found = find(child, path)
    if (found) return found
  }
  return undefined
}

export function demoChildren(path: string): Entry[] {
  const node = find(demoRoot, path) ?? demoRoot
  return (node.children ?? []).map(({ children: _children, ...entry }) => entry).sort((a, b) => b.size - a.size)
}

export function demoBiggest(folders: boolean): Entry[] {
  const files: Entry[] = []
  const dirs: Entry[] = []
  const walk = (node: Node) => {
    if (node.dir && node.path !== demoRoot.path) {
      const { children: _children, ...entry } = node
      dirs.push(entry)
    }
    for (const child of node.children ?? []) {
      if (child.dir) walk(child)
      else files.push(child)
    }
  }
  walk(demoRoot)
  return (folders ? dirs : files).sort((a, b) => b.size - a.size).slice(0, 8)
}

export const demoClean: CleanCategory[] = [
  { id: "temp", title: "arquivos temporários", detail: "pastas temp do usuário e do windows. apagados de vez, sem ir para a lixeira.", needsAdmin: true, bytes: 1_840_000_000, files: 4210, measured: true, available: true, checked: true },
  { id: "chrome", title: "cache do chrome", detail: "só o cache. cookies, histórico e senhas ficam.", needsAdmin: false, bytes: 3_400_000_000, files: 18600, measured: true, available: true, checked: true },
  { id: "edge", title: "cache do edge", detail: "só o cache. cookies, histórico e senhas ficam.", needsAdmin: false, bytes: 920_000_000, files: 6400, measured: true, available: true, checked: true },
  { id: "brave", title: "cache do brave", detail: "só o cache. cookies, histórico e senhas ficam.", needsAdmin: false, bytes: 0, files: 0, measured: true, available: false, checked: false },
  { id: "firefox", title: "cache do firefox", detail: "cache dos perfis do firefox. o histórico e as senhas ficam.", needsAdmin: false, bytes: 640_000_000, files: 2100, measured: true, available: true, checked: true },
  { id: "opera", title: "cache do opera", detail: "cache do opera e do opera gx.", needsAdmin: false, bytes: 0, files: 0, measured: true, available: false, checked: false },
  { id: "recycle", title: "lixeira", detail: "esvazia a lixeira do usuário.", needsAdmin: false, bytes: 2_200_000_000, files: 38, measured: true, available: true, checked: false },
  { id: "winupdate", title: "arquivos antigos do windows update", detail: "esvazia a pasta de download do windows update. pede administrador.", needsAdmin: true, bytes: 4_100_000_000, files: 860, measured: true, available: true, checked: false },
  { id: "dism", title: "limpeza de componentes", detail: "roda o dism startcomponentcleanup. não usa resetbase. pede administrador.", needsAdmin: true, bytes: 0, files: 0, measured: false, available: true, checked: false },
  { id: "logs", title: "logs do windows", detail: "logs do windows update e do cbs. o windows cria de novo. pede administrador.", needsAdmin: true, bytes: 780_000_000, files: 42, measured: true, available: true, checked: false },
  { id: "thumbs", title: "miniaturas", detail: "cache de miniaturas e ícones do explorador. o windows recria ao abrir as pastas.", needsAdmin: false, bytes: 410_000_000, files: 18, measured: true, available: true, checked: true },
  { id: "dumps", title: "despejos de memória", detail: "relatórios de falha do usuário e minidumps, quando der para ler.", needsAdmin: true, bytes: 260_000_000, files: 7, measured: true, available: true, checked: true },
]

export const demoDupes: DupeGroup[] = [
  {
    id: "d1",
    kind: "igual",
    bytes: 12_400_000_000,
    files: [
      { path: "C:\\Users\\ana\\Videos\\ensaio.mov", size: 12_400_000_000, modifiedMs: 1_700_000_000_000, keep: true, distance: null },
      { path: "C:\\Users\\ana\\Videos\\ensaio-copia.mov", size: 12_400_000_000, modifiedMs: 1_720_000_000_000, keep: false, distance: null },
      { path: "D:\\backup\\ensaio.mov", size: 12_400_000_000, modifiedMs: 1_730_000_000_000, keep: false, distance: null },
    ],
  },
  {
    id: "d2",
    kind: "igual",
    bytes: 4_800_000_000,
    files: [
      { path: "C:\\Users\\ana\\Downloads\\aula-de-calculo.mp4", size: 4_800_000_000, modifiedMs: 1_710_000_000_000, keep: true, distance: null },
      { path: "C:\\Users\\ana\\Downloads\\aula-de-calculo (1).mp4", size: 4_800_000_000, modifiedMs: 1_740_000_000_000, keep: false, distance: null },
    ],
  },
  {
    id: "i1",
    kind: "imagem",
    bytes: 9_200_000,
    files: [
      { path: "C:\\Users\\ana\\Pictures\\praia.jpg", size: 9_200_000, modifiedMs: 1_690_000_000_000, keep: true, distance: 4 },
      { path: "C:\\Users\\ana\\Pictures\\praia-editada.jpg", size: 8_800_000, modifiedMs: 1_695_000_000_000, keep: false, distance: 4 },
    ],
  },
]

export const demoHealth: DiskHealth[] = [
  {
    name: "samsung ssd 980 pro 1tb",
    media: "ssd",
    health: "saudável",
    tempC: 41,
    hours: 2410,
    wear: 6,
    tbwGb: 18400,
    readErrors: 0,
    writeErrors: 0,
    smartOk: true,
    size: 1_000_204_886_016,
    source: "msft",
    alert: null,
  },
  {
    name: "seagate barracuda 2tb",
    media: "hdd",
    health: "warning",
    tempC: 54,
    hours: 28640,
    wear: null,
    tbwGb: null,
    readErrors: 18,
    writeErrors: 2,
    smartOk: false,
    size: 2_000_398_934_016,
    source: "smartctl",
    alert: "este disco não está saudável",
  },
]

export const demoStartup: StartupItem[] = [
  { id: "run:hkcu:Spotify", name: "spotify", command: "C:\\Users\\ana\\AppData\\Roaming\\Spotify\\Spotify.exe --autostart", source: "registro do usuário", enabled: true, impact: "alto", canToggle: true, needsAdmin: false },
  { id: "run:hkcu:Discord", name: "discord", command: "C:\\Users\\ana\\AppData\\Local\\Discord\\Update.exe --processStart Discord.exe", source: "registro do usuário", enabled: true, impact: "alto", canToggle: true, needsAdmin: false },
  { id: "run:hkcu:OneDrive", name: "onedrive", command: "C:\\Users\\ana\\AppData\\Local\\Microsoft\\OneDrive\\OneDrive.exe", source: "registro do usuário", enabled: true, impact: "alto", canToggle: true, needsAdmin: false },
  { id: "run:hkcu:AdobeGC", name: "adobe gc invoker", command: "C:\\Program Files\\Common Files\\Adobe\\GC\\AGSService.exe", source: "registro da máquina", enabled: true, impact: "médio", canToggle: true, needsAdmin: true },
  { id: "folder:startup", name: "flow launcher.lnk", command: "C:\\Users\\ana\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\Flow Launcher.lnk", source: "pasta de inicialização", enabled: false, impact: "baixo", canToggle: true, needsAdmin: false },
  { id: "svc:Example", name: "example updater", command: "C:\\Program Files\\Example\\updater.exe", source: "serviço", enabled: true, impact: "médio", canToggle: true, needsAdmin: true },
]

export const demoMonitor: MonitorSample = {
  cpu: 28,
  ramUsed: 11.2 * 1024 ** 3,
  ramTotal: 16 * 1024 ** 3,
  diskUsed: 612 * 1024 ** 3,
  diskTotal: 1000 * 1024 ** 3,
  diskBps: 42 * 1024 ** 2,
  gpu: 14,
  gpuName: "nvidia geforce rtx 4060",
  netDown: 2.4 * 1024 ** 2,
  netUp: 180 * 1024,
  tempC: 47,
  processes: [
    { pid: 4412, name: "chrome", cpu: 11.4, ram: 1.8 * 1024 ** 3 },
    { pid: 2201, name: "bob-tudo", cpu: 4.2, ram: 180 * 1024 ** 2 },
    { pid: 1180, name: "code", cpu: 3.6, ram: 940 * 1024 ** 2 },
    { pid: 804, name: "explorer", cpu: 1.1, ram: 210 * 1024 ** 2 },
    { pid: 990, name: "spotify", cpu: 0.8, ram: 320 * 1024 ** 2 },
    { pid: 1502, name: "onedrive", cpu: 0.4, ram: 140 * 1024 ** 2 },
  ],
}

export const demoPrivacy: PrivacyItem[] = [
  { id: "ads-id", title: "bloquear id de publicidade", detail: "o windows deixa de usar um identificador para anúncios dos aplicativos.", needsAdmin: false, enabled: true },
  { id: "tailored", title: "bloquear experiências sob medida", detail: "para de usar dados de diagnóstico para dicas personalizadas.", needsAdmin: false, enabled: false },
  { id: "suggestions", title: "bloquear sugestões e anúncios", detail: "tira dicas, aplicativos sugeridos e anúncios do menu iniciar e das configurações.", needsAdmin: false, enabled: true },
  { id: "bing", title: "desligar busca na web e cortana", detail: "a pesquisa do iniciar deixa de consultar a web e a cortana.", needsAdmin: false, enabled: false },
  { id: "copilot", title: "esconder o copilot", detail: "esconde o botão do copilot. não desinstala o aplicativo.", needsAdmin: false, enabled: true },
  { id: "activity", title: "desligar histórico de atividades", detail: "para a linha do tempo e a coleta do histórico de atividades.", needsAdmin: false, enabled: false },
  { id: "lockscreen", title: "desligar dicas na tela de bloqueio", detail: "tira as dicas e os cartões da tela de bloqueio.", needsAdmin: false, enabled: true },
  { id: "feedback", title: "desligar pedidos de feedback", detail: "o windows para de pedir avaliações e comentários.", needsAdmin: false, enabled: false },
  { id: "location", title: "negar localização dos aplicativos", detail: "os aplicativos deixam de ter permissão geral de localização.", needsAdmin: false, enabled: false },
  { id: "inking", title: "limitar coleta de tinta e digitação", detail: "o windows para de aprender com o que você escreve.", needsAdmin: false, enabled: false },
  { id: "telemetry", title: "reduzir telemetria", detail: "pede administrador. deixa o diagnóstico enviado à microsoft no mínimo.", needsAdmin: true, enabled: false },
  { id: "start-recs", title: "desligar recomendações do iniciar", detail: "remove as recomendações de arquivos e sites no menu iniciar.", needsAdmin: false, enabled: true },
]

export const demoPoints: RestorePoint[] = [
  { sequence: 18, description: "bob tudo: privacidade", when: "2026-11-02T14:10:00.000Z" },
  { sequence: 12, description: "windows update", when: "2026-10-18T09:02:00.000Z" },
]

export const demoChanges: Change[] = [
  { id: "c1", at: "2026-11-04T12:40:00.000Z", summary: "ligou bloquear sugestões e anúncios", undone: false },
  { id: "c2", at: "2026-11-03T18:12:00.000Z", summary: "desligou spotify na inicialização", undone: false },
]

export const demoSearch: WingetRow[] = [
  { name: "vlc media player", id: "VideoLAN.VLC", version: "3.0.21", available: "", source: "winget" },
  { name: "mpv", id: "mpv.net", version: "0.1.0", available: "", source: "winget" },
  { name: "k-lite codec pack", id: "CodecGuide.K-LiteCodecPack.Standard", version: "18.6.5", available: "", source: "winget" },
]

export const demoOss: OssApp[] = [
  { name: "vlc", description: "abre quase qualquer vídeo e áudio.", category: "vídeo", repo: "videolan/vlc", wingetId: "VideoLAN.VLC", alternativeTo: "alternative ao windows media player", stars: 16200 },
  { name: "7-zip", description: "compacta e abre zip, 7z e rar.", category: "arquivos", repo: "ip7z/7zip", wingetId: "7zip.7zip", alternativeTo: "alternative ao winrar", stars: 4200 },
  { name: "krita", description: "desenho e pintura.", category: "imagem", repo: "KDE/krita", wingetId: "KDE.Krita", alternativeTo: "alternative ao photoshop", stars: 8900 },
  { name: "keepassxc", description: "guarda senhas num cofre local.", category: "segurança", repo: "keepassxreboot/keepassxc", wingetId: "KeePassXCTeam.KeePassXC", alternativeTo: "alternative ao 1password", stars: 24600 },
  { name: "libreoffice", description: "texto, planilha e apresentação.", category: "escritório", repo: "LibreOffice/core", wingetId: "TheDocumentFoundation.LibreOffice", alternativeTo: "alternative ao microsoft office", stars: 3100 },
  { name: "obs studio", description: "grava a tela e transmite ao vivo.", category: "vídeo", repo: "obsproject/obs-studio", wingetId: "OBSProject.OBSStudio", alternativeTo: "", stars: 65400 },
]

export const demoUpgrades: WingetRow[] = [
  { name: "firefox", id: "Mozilla.Firefox", version: "133.0", available: "134.0.1", source: "winget" },
  { name: "vlc media player", id: "VideoLAN.VLC", version: "3.0.20", available: "3.0.21", source: "winget" },
  { name: "git", id: "Git.Git", version: "2.47.0", available: "2.47.1", source: "winget" },
]

export const demoApps: InstalledApp[] = [
  { id: "HKCU\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Discord", name: "discord", version: "1.0.9164", publisher: "discord inc.", location: "C:\\Users\\ana\\AppData\\Local\\Discord", uninstall: "update.exe --uninstall", quiet: "", wingetId: "Discord.Discord", source: "registro", size: 240 * 1024 ** 2 },
  { id: "HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\VLC", name: "vlc media player", version: "3.0.20", publisher: "videolan", location: "C:\\Program Files\\VideoLAN\\VLC", uninstall: "uninstall.exe", quiet: "uninstall.exe /S", wingetId: "VideoLAN.VLC", source: "winget", size: 120 * 1024 ** 2 },
  { id: "HKCU\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Spotify", name: "spotify", version: "1.2.45", publisher: "spotify ab", location: "C:\\Users\\ana\\AppData\\Roaming\\Spotify", uninstall: "spotify.exe /uninstall", quiet: "", wingetId: "Spotify.Spotify", source: "registro", size: 310 * 1024 ** 2 },
]

export const demoLeftovers: Leftover[] = [
  { path: "C:\\Users\\ana\\AppData\\Roaming\\Spotify", kind: "pasta", bytes: 86 * 1024 ** 2 },
  { path: "C:\\Users\\ana\\AppData\\Local\\Spotify", kind: "pasta", bytes: 14 * 1024 ** 2 },
  { path: "registro:HKCU\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Spotify", kind: "registro", bytes: 0 },
]
