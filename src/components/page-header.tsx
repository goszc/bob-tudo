export function PageHeader({ title, sub }: { title: string; sub: string }) {
  return (
    <header className="mb-8">
      <h1 className="text-[32px] font-medium leading-none tracking-[-0.04em]">{title}</h1>
      <p className="mt-3 max-w-[46ch] text-[15px] leading-relaxed text-muted">{sub}</p>
    </header>
  )
}
