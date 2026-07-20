import { X } from "lucide-react";

interface Props {
  open: boolean;
  onClose: () => void;
  hideThisSession: boolean;
  onHideThisSession: (v: boolean) => void;
}

export function SafetyDrawer({ open, onClose, hideThisSession, onHideThisSession }: Props) {
  if (!open) return null;
  return (
    <div className="drawer-backdrop" onClick={onClose} role="presentation">
      <aside
        className="drawer"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-label="安全边界"
      >
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <strong style={{ fontSize: 16 }}>安全边界</strong>
          <button
            type="button"
            className="btn btn-sm btn-ghost"
            onClick={onClose}
            aria-label="关闭"
          >
            <X size={16} />
          </button>
        </div>

        <section style={{ marginTop: 18 }}>
          <div className="section-title">会做什么</div>
          <ul className="secondary" style={{ paddingLeft: 18, lineHeight: 1.6, fontSize: 13 }}>
            <li>只读扫描本机 CC Switch 数据库</li>
            <li>用 Rust HTTP 客户端向同一 Host 发送最小模型请求</li>
            <li>在预算内尝试 URL / 协议 / 模型变体</li>
            <li>给出可在 CC Switch 中手动修改的建议</li>
          </ul>
        </section>

        <section style={{ marginTop: 16 }}>
          <div className="section-title">绝不会做什么</div>
          <ul className="secondary" style={{ paddingLeft: 18, lineHeight: 1.6, fontSize: 13 }}>
            <li>不启动 Codex / Claude / OpenCode / Gemini CLI / CC Switch</li>
            <li>不读取 `.codex` / `.claude` / OpenCode / Gemini 登录目录</li>
            <li>不写入 CC Switch 数据库、不切换供应商</li>
            <li>不保存 Key、选择、结果或历史</li>
            <li>不提供托管登录 / OAuth 绕过</li>
          </ul>
        </section>

        <section style={{ marginTop: 16 }}>
          <div className="section-title">Key 如何处理</div>
          <p className="secondary" style={{ fontSize: 13, lineHeight: 1.55 }}>
            完整 Key 只存在 Rust 内存，用于向原 Base URL 的同一 Host 发请求。前端仅显示脱敏摘要。
          </p>
        </section>

        <section style={{ marginTop: 16 }}>
          <div className="section-title">证据边界</div>
          <p className="secondary" style={{ fontSize: 13, lineHeight: 1.55 }}>
            本工具验证的是<strong>上游 HTTP API</strong>是否可用。它不能证明 Codex/Claude CLI
            端到端完整链路（本地路由、客户端特有头等）。若 Chat 可用而 Responses
            不可用，会标注为可能需要 CC Switch 本地路由。
          </p>
        </section>

        <label
          style={{
            display: "flex",
            gap: 8,
            alignItems: "center",
            marginTop: 22,
            fontSize: 13,
            color: "var(--text-muted)",
          }}
        >
          <input
            type="checkbox"
            checked={hideThisSession}
            onChange={(e) => onHideThisSession(e.target.checked)}
          />
          本次会话不再显示安全提示
        </label>

        <button
          type="button"
          className="btn btn-primary"
          style={{ width: "100%", marginTop: 16 }}
          onClick={onClose}
        >
          知道了
        </button>
      </aside>
    </div>
  );
}
