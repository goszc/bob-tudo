import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import "@fontsource-variable/outfit"
import "./index.css"
import App from "./App"
import { TrayPopup } from "./tray-popup"

const tray = new URLSearchParams(location.search).get("view") === "tray"

createRoot(document.getElementById("root")!).render(
  tray ? <TrayPopup /> : (
    <StrictMode>
      <App />
    </StrictMode>
  ),
)
