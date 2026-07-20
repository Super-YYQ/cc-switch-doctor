import { Search, MoreHorizontal, X } from "lucide-react";
import type { ProviderListItem } from "@/types";
import { ProviderRow } from "./ProviderRow";

const ALL_FILTERS: { id: string; label: string }[] = [
  { id: "all", label: "全部" },
  { id: "claude", label: "Claude" },
  { id: "claude-desktop", label: "Claude Desktop" },
  { id: "codex", label: "Codex" },
  { id: "gemini", label: "Gemini" },
  { id: "opencode", label: "OpenCode" },
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
  onAppFilter: (id: string) => void;
  onQuery: (q: string) => void;
  onOnlySelected: (v: boolean) => void;
  onToggle: (p: ProviderListItem) => void;
  onActivate: (id: string) => void;
  onSelectFiltered: () => void;
  onClearSelection: () => void;
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
  onAppFilter,
  onQuery,
  onOnlySelected,
  onToggle,
  onActivate,
  onSelectFiltered,
  onClearSelection,
}: Props) {
  const presentApps = new Set(providers.map((p) => p.appType));
  const filters = ALL_FILTERS.filter((f) => f.id === "all" || presentApps.has(f.id as never));

  return (
    <section className="panel workspace-pane" style={{ padding: 12 }}>
      <div className="section-title" style={{ marginBottom: 10 }}>
        Provider 配置
      </div>

      <div style={{ display: "flex", gap: 6, flexWrap: "wrap", marginBottom: 10 }}>
        {filters.map((f) => (
          <button
            key={f.id}
            type="button"
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
        <details style={{ position: "relative" }}>
          <summary className="btn btn-sm" style={{ listStyle: "none", cursor: "pointer" }}>
            <MoreHorizontal size={14} />
          </summary>
          <div
            className="panel"
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
              disabled={running}
              onClick={onSelectFiltered}
            >
              全选当前筛选
            </button>
            <button
              className="btn btn-sm"
              type="button"
              disabled={running}
              onClick={onClearSelection}
            >
              取消全选
            </button>
            <button className="btn btn-sm" type="button" onClick={() => onQuery("")}>
              清除搜索
            </button>
          </div>
        </details>
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
            disabled={running}
            onToggle={() => onToggle(p)}
            onActivate={() => onActivate(p.opaqueId)}
          />
        ))}
        {!filtered.length && (
          <div className="empty-state" style={{ minHeight: 180 }}>
            没有匹配的配置
          </div>
        )}
      </div>
    </section>
  );
}
