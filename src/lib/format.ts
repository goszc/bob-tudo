export function bytes(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return "—"
  const sign = value < 0 ? "-" : ""
  let size = Math.abs(value)
  const units = ["b", "kb", "mb", "gb", "tb"]
  let index = 0
  while (size >= 1024 && index < units.length - 1) {
    size /= 1024
    index += 1
  }
  const digits = index === 0 || size >= 10 ? 0 : 1
  const text = new Intl.NumberFormat("pt-BR", { maximumFractionDigits: digits, minimumFractionDigits: digits }).format(size)
  return `${sign}${text} ${units[index]}`
}

export function pct(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return "—"
  return `${new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 0 }).format(value)}%`
}

export function rate(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return "—"
  return `${bytes(value)}/s`
}

export function hours(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return "—"
  const days = Math.round(value / 24)
  return `${new Intl.NumberFormat("pt-BR").format(Math.round(value))} h · ${new Intl.NumberFormat("pt-BR").format(days)} dias`
}

export function stars(value: number): string {
  if (!value) return "—"
  if (value >= 1000) {
    const text = new Intl.NumberFormat("pt-BR", { maximumFractionDigits: value >= 10000 ? 0 : 1 }).format(value / 1000)
    return `${text} mil`
  }
  return new Intl.NumberFormat("pt-BR").format(value)
}

export function when(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return date.toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" })
}
