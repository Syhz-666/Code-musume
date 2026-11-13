import type { BanterConfig, BanterStatus, BanterUsage } from "@code-musume/protocol";
import { useEffect, useState } from "react";
import { invoke } from "./bridge";

function Usage({ usage }: { usage: BanterUsage }) {
  const number = (value: number | null) => value ?? "未知";
  const reuse =
    usage.cacheRead !== null && usage.totalInput
      ? `${((usage.cacheRead / usage.totalInput) * 100).toFixed(1)}%`
      : "未知";
  return (
    <p>
      总输入 {number(usage.totalInput)} · 普通输入 {number(usage.ordinaryInput)} · 缓存读取{" "}
      {number(usage.cacheRead)} · 缓存写入 {number(usage.cacheWrite)} · 输出 {number(usage.output)}{" "}
      · 缓存读取占比 {reuse} · {usage.elapsedMs} 毫秒
    </p>
  );
}

export function BanterSettings() {
  const [status, setStatus] = useState<BanterStatus>();
  const [draft, setDraft] = useState<BanterConfig>();
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [feedback, setFeedback] = useState("");
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let stopped = false;
    async function refresh() {
      try {
        const next = await invoke<BanterStatus>("get_banter_status");
        if (!stopped) {
          setStatus(next);
          setDraft((old) => old ?? next.config);
        }
      } catch {
        if (!stopped) {
          setFailed(true);
          setFeedback("无法读取碎碎念设置");
        }
      }
    }
    void refresh();
    const timer = setInterval(refresh, 2_000);
    return () => {
      stopped = true;
      clearInterval(timer);
    };
  }, []);

  if (!draft || !status) return <p role="status">{feedback || "正在读取设置…"}</p>;
  const sameAddress =
    draft.protocol === status.config.protocol &&
    draft.baseUrl.trim().replace(/\/+$/, "") === status.config.baseUrl;
  const endpoint = `${draft.baseUrl.trim().replace(/\/+$/, "")}/${draft.protocol === "anthropic" ? "messages" : "chat/completions"}`;

  async function run(action: "save" | "delete" | "test" | "preview") {
    setBusy(true);
    setFailed(false);
    setFeedback("");
    try {
      if (action === "test" || action === "preview") {
        const result = await invoke<string>("probe_banter", {
          config: draft,
          key: key || null,
          preview: action === "preview",
        });
        setFeedback(action === "preview" ? `台词预览 · ${result}` : result);
      } else {
        const result = await invoke<BanterStatus>("save_banter", {
          config: draft,
          key: action === "save" ? key || null : null,
          deleteKey: action === "delete",
        });
        setStatus(result);
        setDraft(result.config);
        setKey("");
        setFeedback(action === "delete" ? "密钥已删除" : "已保存");
      }
    } catch (error) {
      setFailed(true);
      setFeedback(typeof error === "string" ? error : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel" data-testid="banter-settings">
      <h2>工作碎碎念</h2>
      <p>开启后，会把选取的工作回复和昵称发送给所填服务。回复可能包含代码或私人信息。</p>
      <fieldset disabled={busy} className="banter-fields">
        <label>
          <input
            type="checkbox"
            checked={draft.enabled}
            onChange={(e) => setDraft({ ...draft, enabled: e.target.checked })}
          />
          开启工作碎碎念
        </label>
        <label>
          接口类型
          <select
            value={draft.protocol}
            onChange={(e) => {
              const protocol = e.target.value === "anthropic" ? "anthropic" : "open-ai";
              setKey("");
              setDraft({ ...draft, protocol, promptCache: false });
            }}
          >
            <option value="open-ai">OpenAI 兼容</option>
            <option value="anthropic">Anthropic 兼容</option>
          </select>
        </label>
        <label>
          API 基础地址
          <input
            value={draft.baseUrl}
            placeholder="https://api.example.com/v1"
            onChange={(e) => {
              setKey("");
              setDraft({ ...draft, baseUrl: e.target.value });
            }}
          />
        </label>
        {draft.baseUrl && <p className="banter-endpoint">请求地址 {endpoint}</p>}
        <label>
          模型名
          <input
            value={draft.model}
            onChange={(e) => setDraft({ ...draft, model: e.target.value })}
          />
        </label>
        <label>
          API Key
          <input
            type="password"
            autoComplete="new-password"
            value={key}
            placeholder={sameAddress && status.hasKey ? "已保存，留空沿用" : "填写该地址的密钥"}
            onChange={(e) => setKey(e.target.value)}
          />
        </label>
        <label>
          昵称
          <input
            value={draft.nickname}
            onChange={(e) => setDraft({ ...draft, nickname: e.target.value })}
          />
        </label>
        {draft.protocol === "anthropic" && (
          <label>
            <input
              type="checkbox"
              checked={draft.promptCache}
              onChange={(e) => setDraft({ ...draft, promptCache: e.target.checked })}
            />
            服务支持 Anthropic 提示缓存
          </label>
        )}
        <div className="actions">
          <button type="button" onClick={() => void run("save")}>
            保存
          </button>
          <button
            type="button"
            disabled={!sameAddress || !status.hasKey}
            onClick={() => void run("delete")}
          >
            删除密钥
          </button>
          <button type="button" onClick={() => void run("test")}>
            测试连接
          </button>
          <button type="button" onClick={() => void run("preview")}>
            角色台词预览
          </button>
        </div>
        <small>测试与预览使用固定样例，可能产生少量费用。</small>
      </fieldset>
      {feedback && <p role={failed ? "alert" : "status"}>{feedback}</p>}
      <p>
        Codex {status.codexReady ? "已收到进展" : "等待进展"} · Claude Code{" "}
        {status.claudeReady ? "已找到会话记录" : "等待会话记录"}
      </p>
      <p>
        {status.outcome} · 超长回复已跳过 {status.skippedLong} 条
      </p>
      {status.usage && <Usage usage={status.usage} />}
    </section>
  );
}
