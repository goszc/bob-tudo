import { mkdir } from "node:fs/promises"
import { chromium } from "playwright"

const themes = [
  ["claro", "light"],
  ["escuro", "dark"],
  ["vidro-claro", "glass-light"],
  ["vidro-escuro", "glass-dark"],
]
const pages = [
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

const dirs = ["docs/screenshots", "/opt/cursor/artifacts/screenshots"]
for (const dir of dirs) await mkdir(dir, { recursive: true })

const browser = await chromium.launch({ args: ["--font-render-hinting=none", "--disable-dev-shm-usage"] })
const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 })
page.on("pageerror", (error) => console.log("PAGE", error.message))

for (const [fileTheme, theme] of themes) {
  for (const name of pages) {
    const scheme = theme === "dark" || theme === "glass-dark" ? "dark" : "light"
    await page.emulateMedia({ colorScheme: scheme })
    await page.goto(`http://127.0.0.1:47331/?preview=1&theme=${theme}&page=${name}`, { waitUntil: "networkidle" })
    await page.waitForSelector("h1")
    await page.evaluate(() => document.fonts.ready)
    await page.waitForTimeout(200)
    for (const dir of dirs) {
      const path = `${dir}/${name}-${fileTheme}.png`
      await page.screenshot({ path })
    }
    console.log("wrote", name, fileTheme)
  }
}

await browser.close()
