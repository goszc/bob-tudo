export type Rect = { x: number; y: number; w: number; h: number }

type Node<T> = { item: T; area: number }

export function treemap<T extends { size: number }>(items: T[], width: number, height: number): Array<T & Rect> {
  const usable = items.filter((item) => item.size > 0)
  const total = usable.reduce((sum, item) => sum + item.size, 0)
  if (!total || width <= 0 || height <= 0) return []
  const nodes = usable.map((item) => ({ item, area: (item.size / total) * width * height }))
  const out: Array<T & Rect> = []
  let box: Rect = { x: 0, y: 0, w: width, h: height }
  let rest = nodes
  while (rest.length && box.w > 1 && box.h > 1) {
    const short = Math.min(box.w, box.h)
    const row = [rest[0]]
    let index = 1
    while (
      index < rest.length &&
      worst(
        row.map((node) => node.area),
        short,
      ) >=
        worst(
          [...row.map((node) => node.area), rest[index].area],
          short,
        )
    ) {
      row.push(rest[index])
      index += 1
    }
    box = place(row, box, out)
    rest = rest.slice(index)
  }
  return out
}

function worst(areas: number[], side: number): number {
  const sum = areas.reduce((left, right) => left + right, 0)
  const max = Math.max(...areas)
  const min = Math.min(...areas)
  const side2 = side * side
  const sum2 = sum * sum
  return Math.max((side2 * max) / sum2, sum2 / (side2 * min))
}

function place<T>(row: Node<T>[], box: Rect, out: Array<T & Rect>): Rect {
  const sum = row.reduce((left, node) => left + node.area, 0)
  const short = Math.min(box.w, box.h)
  const thickness = sum / short
  if (box.w >= box.h) {
    let y = box.y
    for (const node of row) {
      const h = node.area / thickness
      out.push({ ...node.item, x: box.x, y, w: thickness, h })
      y += h
    }
    return { x: box.x + thickness, y: box.y, w: Math.max(0, box.w - thickness), h: box.h }
  }
  let x = box.x
  for (const node of row) {
    const w = node.area / thickness
    out.push({ ...node.item, x, y: box.y, w, h: thickness })
    x += w
  }
  return { x: box.x, y: box.y + thickness, w: box.w, h: Math.max(0, box.h - thickness) }
}

export function kindColor(kind: string): string {
  switch (kind) {
    case "imagem":
      return "#e2b15a"
    case "video":
      return "#7f9a62"
    case "audio":
      return "#6a93a8"
    case "arquivo":
      return "#c48462"
    case "documento":
      return "#9a86b8"
    case "codigo":
      return "#6d917c"
    case "programa":
      return "#c46b6b"
    case "pasta":
      return "#8aa4c4"
    default:
      return "#b7b3aa"
  }
}
