import { lazy, Suspense, useEffect, useState, type ReactNode } from "react"
import { Shell } from "./components/shell"
import { HomePage } from "./pages/home"
import { initApp, useApp } from "./store"
import type { Page } from "./lib/types"

const DiskPage = lazy(() => import("./pages/disk").then((mod) => ({ default: mod.DiskPage })))
const CleanPage = lazy(() => import("./pages/clean").then((mod) => ({ default: mod.CleanPage })))
const DupesPage = lazy(() => import("./pages/dupes").then((mod) => ({ default: mod.DupesPage })))
const HealthPage = lazy(() => import("./pages/health").then((mod) => ({ default: mod.HealthPage })))
const StartupPage = lazy(() => import("./pages/startup").then((mod) => ({ default: mod.StartupPage })))
const MonitorPage = lazy(() => import("./pages/monitor").then((mod) => ({ default: mod.MonitorPage })))
const PrivacyPage = lazy(() => import("./pages/privacy").then((mod) => ({ default: mod.PrivacyPage })))
const RestorePage = lazy(() => import("./pages/restore").then((mod) => ({ default: mod.RestorePage })))
const CatalogPage = lazy(() => import("./pages/catalog").then((mod) => ({ default: mod.CatalogPage })))
const UpgradesPage = lazy(() => import("./pages/upgrades").then((mod) => ({ default: mod.UpgradesPage })))
const UninstallPage = lazy(() => import("./pages/uninstall").then((mod) => ({ default: mod.UninstallPage })))
const SettingsPage = lazy(() => import("./pages/settings").then((mod) => ({ default: mod.SettingsPage })))
const SearchPage = lazy(() => import("./pages/search").then((mod) => ({ default: mod.SearchPage })))
const HistoryPage = lazy(() => import("./pages/history").then((mod) => ({ default: mod.HistoryPage })))
const BackupPage = lazy(() => import("./pages/backup").then((mod) => ({ default: mod.BackupPage })))
const DriversPage = lazy(() => import("./pages/system-plus").then((mod) => ({ default: mod.DriversPage })))
const NetworkPage = lazy(() => import("./pages/system-plus").then((mod) => ({ default: mod.NetworkPage })))
const GamePage = lazy(() => import("./pages/system-plus").then((mod) => ({ default: mod.GamePage })))
const PowerPage = lazy(() => import("./pages/system-plus").then((mod) => ({ default: mod.PowerPage })))
const ReportPage = lazy(() => import("./pages/report").then((mod) => ({ default: mod.ReportPage })))

const PAGES: { id: Page; node: (active: boolean) => ReactNode }[] = [
  { id: "home", node: () => <HomePage /> },
  { id: "disk", node: () => <DiskPage /> },
  { id: "search", node: () => <SearchPage /> },
  { id: "history", node: () => <HistoryPage /> },
  { id: "clean", node: () => <CleanPage /> },
  { id: "dupes", node: () => <DupesPage /> },
  { id: "health", node: () => <HealthPage /> },
  { id: "startup", node: () => <StartupPage /> },
  { id: "monitor", node: (active) => <MonitorPage active={active} /> },
  { id: "drivers", node: () => <DriversPage /> },
  { id: "network", node: () => <NetworkPage /> },
  { id: "game", node: () => <GamePage /> },
  { id: "power", node: () => <PowerPage /> },
  { id: "privacy", node: () => <PrivacyPage /> },
  { id: "restore", node: () => <RestorePage /> },
  { id: "catalog", node: () => <CatalogPage /> },
  { id: "backup", node: () => <BackupPage /> },
  { id: "upgrades", node: () => <UpgradesPage /> },
  { id: "uninstall", node: () => <UninstallPage /> },
  { id: "settings", node: () => <SettingsPage /> },
  { id: "report", node: () => <ReportPage /> },
]

export default function App() {
  const page = useApp().page
  const [seen, setSeen] = useState<Page[]>(["home"])
  useEffect(() => {
    initApp()
  }, [])
  useEffect(() => {
    setSeen((current) => (current.includes(page) ? current : [...current, page]))
  }, [page])
  return (
    <Shell>
      {PAGES.map((item) => {
        if (!seen.includes(item.id) && item.id !== "home") return null
        const active = page === item.id
        return (
          <div key={item.id} className={active ? "absolute inset-0 overflow-auto" : "hidden"} hidden={!active}>
            <div className="mx-auto w-full max-w-[1080px] px-5 py-8 md:px-10">
              <Suspense fallback={<p className="text-[13px] text-muted">abrindo…</p>}>{item.node(active)}</Suspense>
            </div>
          </div>
        )
      })}
    </Shell>
  )
}
