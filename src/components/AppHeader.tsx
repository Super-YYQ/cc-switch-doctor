import { RefreshCw, FolderOpen, Shield, CircleHelp } from "lucide-react";
import type { AppInfo, ProviderScanView, UpdateStatus } from "@/types";
import { schemaKind, schemaLabel } from "@/lib/utils";

interface Props {
  appInfo: AppInfo | null;
  scan: ProviderScanView | null;
  updates: UpdateStatus | null;
  running: boolean;
  onRefresh: () => void;
  onPickDb: () => void;
  onCheckUpdates: () => void;
  onOpenSafety: () => void;
}

function updateMessage(updates: UpdateStatus | null): string | null {
  if (updates?.message) return updates.message;
  return null;
}

export function AppHeader({
  appInfo,
  scan,
  updates,
  running,
  onRefresh,
  onPickDb,
  onCheckUpdates,
  onOpenSafety,
}: Props) {
  const schema = scan?.schema?.status;
  const observed = updates?.ccSwitchLatest ?? scan?.ccSwitchVersionHint ?? null;
  const verified = updates?.verifiedCcSwitch ?? null;

  return (
    <header className="panel app-header" style={{ padding: "8px 12px" }}>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          gap: 10,
          alignItems: "center",
          flexWrap: "wrap",
        }}
      >
        <div style={{ display: "flex", gap: 10, alignItems: "center", minWidth: 0 }}>
          <div
            style={{
              width: 32,
              height: 32,
              borderRadius: 10,
              background: "var(--primary-soft)",
              color: "var(--primary)",
              display: "grid",
              placeItems: "center",
              flexShrink: 0,
            }}
          >
            <Shield size={16} />
          </div>
          <div style={{ minWidth: 0 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 6, flexWrap: "wrap" }}>
              <strong style={{ fontSize: 15 }}>CC Switch Doctor</strong>
              <span className="badge primary">v{appInfo?.doctorVersion ?? "0.1.4"}</span>
            </div>
            <div className="muted" style={{ fontSize: 11, marginTop: 1 }}>
              只读扫描 · 纯 HTTP · 不启动 AI CLI
            </div>
          </div>
        </div>

        <div style={{ display: "flex", gap: 8, flexWrap: "wrap", alignItems: "center" }}>
          <span className={`badge ${scan?.discovery.found ? "ok" : "warn"}`}>
            {scan?.discovery.found ? "DB 已连接" : "DB 未连接"}
          </span>
          <span className={`badge ${schemaKind(schema)}`}>{schemaLabel(schema)}</span>
          {observed && <span className="badge">CC Switch 最新：{observed}</span>}
          {verified && <span className="badge ok">Doctor 已验证：{verified}</span>}
          <button className="btn btn-ghost btn-sm" type="button" onClick={onOpenSafety}>
            <CircleHelp size={14} /> 安全边界
          </button>
          <button className="btn btn-sm" type="button" onClick={onCheckUpdates}>
            检查更新
          </button>
          <button
            className="btn btn-sm"
            type="button"
            disabled={running}
            onClick={onRefresh}
            aria-label="刷新配置"
          >
            <RefreshCw size={14} /> 刷新
          </button>
          <button
            className="btn btn-sm"
            type="button"
            disabled={running}
            onClick={onPickDb}
            aria-label="选择数据库"
          >
            <FolderOpen size={14} /> 选择 DB
          </button>
        </div>
      </div>
      {updateMessage(updates) && (
        <div className="muted" style={{ marginTop: 8, fontSize: 12 }}>
          {updateMessage(updates)}
        </div>
      )}
      {scan?.schema?.message && (
        <div className="muted" style={{ marginTop: 6, fontSize: 12 }}>
          {scan.schema.message}
        </div>
      )}
    </header>
  );
}
