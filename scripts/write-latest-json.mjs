import { readFileSync, writeFileSync } from "node:fs"

const version = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8")).version
const signature = readFileSync(new URL("../packaging/bob-tudo-setup.exe.sig", import.meta.url), "utf8").trim()
const repo = process.env.GITHUB_REPOSITORY || "goszc/bob-tudo"
const url = process.env.UPDATE_ASSET_URL || `https://github.com/${repo}/releases/download/v${version}/bob-tudo-setup.exe`

const manifest = {
  version,
  notes: "bob tudo",
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": { signature, url },
  },
}

writeFileSync(new URL("../packaging/latest.json", import.meta.url), JSON.stringify(manifest, null, 2))
console.log("latest.json", version)
