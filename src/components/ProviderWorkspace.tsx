import { useEffect, useRef, useState } from "react";
import { Search, MoreHorizontal, X } from "lucide-react";
import type { ProviderListItem } from "@/types";
import { ProviderRow } from "./ProviderRow";

/** Always show core app filters even when the current DB has zero matching rows. */
const CORE_FILTERS: { id: string; label: string }[] = [
  { id: "all", label: "全部" },
  { id: "claude", label: "Claude" },
  { id: "codex", label: "Codex" },
  { id: "gemini", label: "Gemini" },
  { id: "opencode", label: "OpenCode" },
];

const EXTRA_FILTERS: { id: string; label: string }[] = [
  { id: "claude-desktop", label: "Claude Desktop" },
  { id: "openclaw", label: "OpenClaw" },
  { id: "hermes", label: "Hermes" },
  { id: "grokbuild", label: "Grok" },
];

interface Props {
  providers: ProviderListItem[];
  filtered: ProviderListItem[];
  appFilter: string;
  query: string;
  onlySelected: boolean;
  selected: Set<string>;
  activeId: string | null;
  runningIds: Set<string>;
  statusById: Map<string, string>;
  running: boolean;
  schemaStatus?: string | null;
  canTest?: boolean;
  onAppFilter: (id: string) => void;
  onQuery: (q: string) => void;
  onOnlySelected: (v: boolean) => void;
  onToggle: (p: ProviderListItem) => void;
  onActivate: (id: string) => void;
  onSelectFiltered: () => void;
  onClearSelection: () => void;
  onSelectCurrent?: () => void;
}

export function ProviderWorkspace({
  providers,
  filtered,
  appFilter,
  query,
  onlySelected,
  selected,
  activeId,
  runningIds,
  statusById,
  running,
  schemaStatus,
  canTest,
  onAppFilter,
  onQuery,
  onOnlySelected,
  onToggle,
  onActivate,
  onSelectFiltered,
  onClearSelection,
  onSelectCurrent,
}: Props) {
  const presentApps = new Set(providers.map((p) => p.appType));
  const filters = [...CORE_FILTERS, ...EXTRA_FILTERS.filter((f) => presentApps.has(f.id as never))];

  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!menuOpen) return;
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (menuRef.current?.contains(target)) return;
      if (triggerRef.current?.contains(target)) return;
      setMenuOpen(false);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setMenuOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [menuOpen]);

  const emptyMessage = (() => {
    // Empty-state guidance: only hard-block when capability says cannot test.
    // Unknown version with usable structure still shows providers.
    if (canTest === false || schemaStatus === "unsupported") {
      return "当前数据库结构缺少 Provider 必需字段，已安全停止读取敏感配置。";
    }
    if (query.trim()) {
      return "没有匹配搜索条件的配置。";
    }
    if (appFilter !== "all") {
      return "当前应用筛选下没有配置。";
    }
    if (onlySelected) {
      return "尚未勾选任何配置。";
    }
    if (providers.length === 0) {
      return "数据库中未找到可展示的第三方配置。";
    }
    return "没有匹配的配置";
  })();

  return (
    <section className="panel workspace-pane" style={{ padding: 12 }}>
      <div className="section-title" style={{ marginBottom: 10 }}>
        Provider 配置
      </div>

      <div
        style={{ display: "flex", gap: 6, flexWrap: "wrap", marginBottom: 10 }}
        role="tablist"
        aria-label="应用筛选"
      >
        {filters.map((f) => (
          <button
            key={f.id}
            type="button"
            role="tab"
            aria-selected={appFilter === f.id}
            className={`chip ${appFilter === f.id ? "active" : ""}`}
            onClick={() => onAppFilter(f.id)}
          >
            {f.label}
          </button>
        ))}
      </div>

      <div style={{ display: "flex", gap: 8, marginBottom: 10, alignItems: "center" }}>
        <div style={{ position: "relative", flex: 1, minWidth: 160 }}>
          <Search size={14} className="muted" style={{ position: "absolute", left: 11, top: 11 }} />
          <input
            className="input"
            value={query}
            onChange={(e) => onQuery(e.target.value)}
            placeholder="搜索供应商 / Host / 模型"
            aria-label="搜索"
          />
          {query && (
            <button
              type="button"
              className="btn-ghost"
              style={{
                position: "absolute",
                right: 6,
                top: 6,
                border: 0,
                background: "transparent",
                color: "var(--text-muted)",
              }}
              onClick={() => onQuery("")}
              aria-label="清除搜索"
            >
              <X size={14} />
            </button>
          )}
        </div>
        <label
          className="muted"
          style={{ display: "inline-flex", gap: 6, alignItems: "center", fontSize: 12 }}
        >
          <input
            type="checkbox"
            checked={onlySelected}
            onChange={(e) => onOnlySelected(e.target.checked)}
          />
          仅看已选
        </label>
        <div style={{ position: "relative" }}>
          <button
            ref={triggerRef}
            type="button"
            className="btn btn-sm"
            aria-label="更多操作"
            aria-expanded={menuOpen}
            aria-haspopup="menu"
            onClick={() => setMenuOpen((v) => !v)}
          >
            <MoreHorizontal size={14} />
          </button>
          {menuOpen && (
            <div
              ref={menuRef}
              className="panel"
              role="menu"
              style={{
                position: "absolute",
                right: 0,
                top: 36,
                zIndex: 5,
                padding: 8,
                minWidth: 160,
                display: "grid",
                gap: 4,
              }}
            >
              <button
                className="btn btn-sm"
                type="button"
                role="menuitem"
                disabled={running}
                onClick={() => {
                  onSelectFiltered();
                  setMenuOpen(false);
                }}
              >
                全选当前筛选
              </button>
              <button
                className="btn btn-sm"
                type="button"
                role="menuitem"
                disabled={running}
                onClick={() => {
                  onClearSelection();
                  setMenuOpen(false);
                }}
              >
                取消全选
              </button>
              {onSelectCurrent && (
                <button
                  className="btn btn-sm"
                  type="button"
                  role="menuitem"
                  disabled={running}
                  onClick={() => {
                    onSelectCurrent();
                    setMenuOpen(false);
                  }}
                >
                  选择 CCS 当前配置
                </button>
              )}
              <button
                className="btn btn-sm"
                type="button"
                role="menuitem"
                onClick={() => {
                  onQuery("");
                  setMenuOpen(false);
                }}
              >
                清除搜索
              </button>
            </div>
          )}
        </div>
      </div>

      <div className="workspace-scroll">
        {filtered.map((p) => (
          <ProviderRow
            key={p.opaqueId}
            provider={p}
            checked={selected.has(p.opaqueId)}
            active={activeId === p.opaqueId}
            running={runningIds.has(p.opaqueId)}
            resultStatus={statusById.get(p.opaqueId)}
            hasResult={statusById.has(p.opaqueId)}
            disabled={running}
            onToggle={() => onToggle(p)}
            onActivate={() => onActivate(p.opaqueId)}
          />
        ))}
        {!filtered.length && (
          <div className="empty-state" style={{ minHeight: 180 }} data-testid="provider-empty">
            {emptyMessage}
          </div>
        )}
      </div>
    </section>
  );
}
