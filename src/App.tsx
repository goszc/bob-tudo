import { useEffect } from "react"
import { Shell } from "./components/shell"
import { CleanPage } from "./pages/clean"
import { DiskPage } from "./pages/disk"
import { DupesPage } from "./pages/dupes"
import { HealthPage } from "./pages/health"
import { InstallPage } from "./pages/install"
import { MonitorPage } from "./pages/monitor"
import { OssPage } from "./pages/oss"
import { PrivacyPage } from "./pages/privacy"
import { RestorePage } from "./pages/restore"
import { SettingsPage } from "./pages/settings"
import { StartupPage } from "./pages/startup"
import { UninstallPage } from "./pages/uninstall"
import { UpgradesPage } from "./pages/upgrades"
import { initApp, useApp } from "./store"

export default function App() {
  const page = useApp().page
  useEffect(() => {
    initApp()
  }, [])
  return (
    <Shell>
      {page === "disk" && <DiskPage />}
      {page === "clean" && <CleanPage />}
      {page === "dupes" && <DupesPage />}
      {page === "health" && <HealthPage />}
      {page === "startup" && <StartupPage />}
      {page === "monitor" && <MonitorPage />}
      {page === "privacy" && <PrivacyPage />}
      {page === "restore" && <RestorePage />}
      {page === "install" && <InstallPage />}
      {page === "oss" && <OssPage />}
      {page === "upgrades" && <UpgradesPage />}
      {page === "uninstall" && <UninstallPage />}
      {page === "settings" && <SettingsPage />}
    </Shell>
  )
}
