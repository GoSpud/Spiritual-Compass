/**
 * Transient overlay shown while EPUBs are being copied + parsed.
 * Kept intentionally simple: import is fast for typical files.
 */
export function ImportDialog() {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: "rgba(0,0,0,0.35)" }}>
      <div
        className="rounded-xl px-6 py-5 themed-surface flex items-center gap-3"
        style={{ background: "var(--bg-elevated)", border: "1px solid var(--border)", boxShadow: "var(--shadow)" }}
      >
        <span className="inline-block h-4 w-4 animate-spin rounded-full border-2 border-t-transparent" style={{ borderColor: "var(--accent)" }} />
        <p className="text-sm">Importing books…</p>
      </div>
    </div>
  );
}
