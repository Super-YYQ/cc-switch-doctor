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
  return (
    <header className="panel" style={{ padding: "12px 16px" }}>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          gap: 12,
          alignItems: "flex-start",
          flexWrap: "wrap",
        }}
      >
        <div style={{ display: "flex", gap: 12, alignItems: "center", minWidth: 0 }}>
          <div
            style={{
              width: 40,
              height: 40,
              borderRadius: 12,
              background: "var(--primary-soft)",
              color: "var(--primary)",
              display: "grid",
              placeItems: "center",
              flexShrink: 0,
            }}
          >
            <Shield size={20} />
          </div>
          <div style={{ minWidth: 0 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
              <strong style={{ fontSize: 18 }}>CC Switch Doctor</strong>
              <span className="badge primary">v{appInfo?.doctorVersion ?? "0.1.1"}</span>
            </div>
            <div className="muted" style={{ fontSize: 12, marginTop: 2 }}>
              只读扫描 · 纯 HTTP · 不启动 AI CLI
            </div>
          </div>
        </div>

        <div style={{ display: "flex", gap: 8, flexWrap: "wrap", alignItems: "center" }}>
          <span className={`badge ${scan?.discovery.found ? "ok" : "warn"}`}>
            {scan?.discovery.found ? "DB 已连接" : "DB 未连接"}
          </span>
          <span className={`badge ${schemaKind(schema)}`}>{schemaLabel(schema)}</span>
          {(scan?.ccSwitchVersionHint || updates?.verifiedCcSwitch) && (
            <span className="badge">
              CC Switch {scan?.ccSwitchVersionHint ?? updates?.verifiedCcSwitch}
            </span>
          )}
          <button className="btn btn-ghost btn-sm" type="button" onClick={onOpenSafety}>
            <CircleHelp size={14} /> 安全边界
          </button>
          <button className="btn btn-sm" type="button" onClick={onCheckUpdates}>
            检查更新
          </button>
          <button className="btn btn-sm" type="button" disabled={running} onClick={onRefresh}>
            <RefreshCw size={14} /> 刷新
          </button>
          <button className="btn btn-sm" type="button" disabled={running} onClick={onPickDb}>
            <FolderOpen size={14} /> 选择 DB
          </button>
        </div>
      </div>
      {updates?.message && (
        <div className="muted" style={{ marginTop: 8, fontSize: 12 }}>
          {updates.message}
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
