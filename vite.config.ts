import tailwindcss from "@tailwindcss/vite"
import react from "@vitejs/plugin-react"
import { defineConfig } from "vite"

export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    host: "0.0.0.0",
    port: 47331,
    strictPort: true,
  },
  preview: {
    host: "0.0.0.0",
    port: 47331,
    strictPort: true,
  },
})
