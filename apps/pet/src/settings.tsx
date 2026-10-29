import {
  type Agent,
  type AnimationPreview,
  AnimationPreviewStore,
  type CharacterManifest,
  type Config,
  type Diagnostic,
  type DisplayState,
  type HookStatus,
  type InstallPreview,
  STATE_LABELS,
} from "@code-musume/protocol";
import { StrictMode, useCallback, useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke, isTauri, preview, watchAnimationPreview, watchProjection } from "./bridge";
import { builtinManifest } from "./builtin-character";
import "./settings.css";

const defaults: Config = {
  schemaVersion: 1,
  characterId: "builtin",
  formLock: null,
  agentForms: {},
  ghost: false,
  userHidden: false,
  reducedMotion: false,
  manualAnimationEnabled: false,
  fullscreenHide: true,
  autostart: false,
  renderer: "pixi",
  position: null,
};
const agentNames: Record<Agent, string> = { "claude-code": "Claude Code", codex: "Codex 桌面" };
function Settings() {
  const [p, setP] = useState(preview),
    [config, setConfig] = useState(defaults);
  const [tab, setTab] = useState("connect"),
    [scope, setScope] = useState("project"),
    [cwd, setCwd] = useState("");
  const [characters, setCharacters] = useState<CharacterManifest[]>([builtinManifest]),
    [statuses, setStatuses] = useState<HookStatus[]>([]),
    [diagnostics, setDiagnostics] = useState<Diagnostic[]>([]);
  const [message, setMessage] = useState(""),
    [error, setError] = useState(""),
    [busy, setBusy] = useState(false),
    [pending, setPending] = useState<InstallPreview | null>(null);
  const [source, setSource] = useState("");
  const [animationPreview, setAnimationPreview] = useState<AnimationPreview | null>(null);
  const previewStore = useRef(new AnimationPreviewStore());
  const acceptAnimationPreview = useCallback((next: AnimationPreview) => {
    if (!previewStore.current.accept(next)) return;
    setAnimationPreview(next);
    setConfig((previous) => ({ ...previous, manualAnimationEnabled: next.enabled }));
  }, []);
  useEffect(() => {
    let cancel = false,
      stop: (() => void) | undefined,
      stopAnimationPreview: (() => void) | undefined;
    void watchProjection(setP)
      .then((f) => {
        if (cancel) f();
        else stop = f;
      })
      .catch((e: unknown) => setError(String(e)));
    void watchAnimationPreview((next) => {
      if (!cancel) acceptAnimationPreview(next);
    })
      .then((f) => {
        if (cancel) f();
        else stopAnimationPreview = f;
      })
      .catch((e: unknown) => setError(String(e)));
    if (isTauri())
      void Promise.all([
        invoke<Config>("get_config"),
        invoke<CharacterManifest[]>("list_characters"),
      ])
        .then(([c, chars]) => {
          if (!cancel) {
            setConfig({
              ...c,
              manualAnimationEnabled:
                previewStore.current.current?.enabled ?? c.manualAnimationEnabled,
            });
            setCharacters(chars);
          }
        })
        .catch((e: unknown) => setError(String(e)));
    return () => {
      cancel = true;
      stop?.();
      stopAnimationPreview?.();
    };
  }, [acceptAnimationPreview]);
  useEffect(() => {
    setPending(null);
    if (isTauri() && (scope === "user" || cwd))
      void Promise.all(
        (["claude-code", "codex"] as Agent[]).map((agent) =>
          invoke<HookStatus>("hook_status", { agent, scope, cwd: cwd || null }),
        ),
      )
        .then(setStatuses)
        .catch((e: unknown) => setError(String(e)));
  }, [scope, cwd]);
  const character = characters.find((m) => m.id === config.characterId);
  const activeCharacter = characters.find((m) => m.id === p.characterId);
  const activeForm =
    activeCharacter?.forms.find((f) => f.id === p.formId) ??
    activeCharacter?.forms.find((f) => f.id === activeCharacter.defaultForm);
  const previewState = animationPreview?.enabled ? animationPreview.state : null;
  const missingPreviewClip = previewState && activeForm && !activeForm.clips[previewState];
  async function task(action: () => Promise<void>) {
    setBusy(true);
    setError("");
    setMessage("");
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  async function save(next = config) {
    await task(async () => {
      setConfig(await invoke<Config>("set_config", { config: next }));
      setMessage("设置已保存");
    });
  }
  async function prepare(agent: Agent, operation: string) {
    await task(async () => {
      setPending(
        await invoke<InstallPreview>("preview_hooks", {
          agent,
          operation,
          scope,
          cwd: cwd || null,
        }),
      );
    });
  }
  async function confirm() {
    const plan = pending;
    if (!plan) return;
    await task(async () => {
      await invoke("apply_hooks", {
        agent: plan.agent,
        operation: plan.operation,
        scope,
        cwd: cwd || null,
        digest: plan.digest,
      });
      setPending(null);
      setStatuses(
        await Promise.all(
          (["claude-code", "codex"] as Agent[]).map((agent) =>
            invoke<HookStatus>("hook_status", { agent, scope, cwd: cwd || null }),
          ),
        ),
      );
      setMessage(
        plan.agent === "codex" && plan.operation === "install"
          ? "接线已写入。请在 Codex 中审阅并信任这些 hook，然后新开本机任务验证。"
          : plan.operation === "install"
            ? "接线已写入，请新开受信任项目中的会话验证。"
            : "已卸载所选接线",
      );
    });
  }
  async function choose(folder: boolean, project = false) {
    await task(async () => {
      const path = await invoke<string | null>("choose_path", { folder });
      if (path) {
        if (project) setCwd(path);
        else setSource(path);
      }
    });
  }
  async function importCharacter() {
    await task(async () => {
      const m = await invoke<CharacterManifest>("import_character", { source });
      setCharacters(await invoke<CharacterManifest[]>("list_characters"));
      setConfig(
        await invoke<Config>("set_config", {
          config: { ...config, characterId: m.id, formLock: null, agentForms: {} },
        }),
      );
      setMessage(`已导入并使用「${m.name}」`);
    });
  }
  return (
    <main
      className="settings-shell"
      data-testid="settings"
      data-state={p.state}
      data-pending={p.pendingCount}
    >
      <header>
        <div className="brand-mark">✦</div>
        <div>
          <h1>代码娘</h1>
          <p>陪你写代码，替工作传个信。</p>
        </div>
        <span className="version">0.1 · Windows</span>
        <button
          type="button"
          className="exit-app"
          data-testid="exit-app"
          disabled={!isTauri()}
          onClick={() => void task(() => invoke<void>("quit_app"))}
        >
          退出代码娘
        </button>
      </header>
      <section className="summary">
        <div>
          <span className={`state-dot ${p.state}`} />
          <strong>{STATE_LABELS[p.state]}</strong>
          <span>{p.dominantAgent ? agentNames[p.dominantAgent] : "等待工作事件"}</span>
        </div>
        <div className="counts">
          <span>{p.activeSessions} 个会话</span>
          <span>{p.pendingCount} 项待处理</span>
        </div>
      </section>
      {!isTauri() && <p className="notice">界面预览。在桌宠应用中打开设置即可连接工作事件。</p>}
      <nav aria-label="设置分类">
        {[
          ["connect", "工作联动"],
          ["character", "角色与形态"],
          ["preferences", "使用偏好"],
          ["diagnostics", "诊断"],
        ].map(([id, name]) => (
          <button
            type="button"
            key={id}
            data-testid={`tab-${id}`}
            className={tab === id ? "active" : ""}
            onClick={() => setTab(id ?? "connect")}
          >
            {name}
          </button>
        ))}
      </nav>
      {message && (
        <p className="notice success" role="status">
          {message}
        </p>
      )}
      {error && (
        <p className="notice error" role="alert">
          {error}
        </p>
      )}
      {tab === "connect" && (
        <>
          <div className="section-title">
            <h2>连接你的工作</h2>
            <p>一次接线，随后让角色跟着真实工作做出反应。</p>
          </div>
          <div className="scope">
            <label>
              使用范围
              <select value={scope} onChange={(e) => setScope(e.target.value)}>
                <option value="user">我的所有项目</option>
                <option value="project">指定项目</option>
              </select>
            </label>
            {scope === "project" && (
              <div className="path-input">
                <input
                  aria-label="项目目录"
                  placeholder="选择项目目录"
                  value={cwd}
                  onChange={(e) => setCwd(e.target.value)}
                />
                <button
                  type="button"
                  disabled={busy || !isTauri()}
                  onClick={() => void choose(true, true)}
                >
                  选择目录
                </button>
              </div>
            )}
          </div>
          <div className="agent-cards">
            {(["claude-code", "codex"] as Agent[]).map((agent) => {
              const s = statuses.find((s) => s.agent === agent);
              const seen = p.sessions.some((s) => s.agent === agent);
              return (
                <article
                  key={agent}
                  className={agent === "codex" ? "agent-card codex" : "agent-card claude"}
                >
                  <div className="card-head">
                    <span className="agent-monogram">{agent === "codex" ? "C" : "✳"}</span>
                    <h3>{agentNames[agent]}</h3>
                    <span className="status-tag">
                      {seen ? "已收到事件" : s?.installed ? "已接线" : "尚未接线"}
                    </span>
                  </div>
                  <p>
                    {agent === "codex"
                      ? "支持当前桌面应用中的本机任务；接线后需在 Codex 内确认信任。"
                      : "支持提交、工具、等待处理和轮次结束等工作状态。"}
                  </p>
                  <div className="actions">
                    <button
                      type="button"
                      disabled={busy || !isTauri()}
                      onClick={() => void prepare(agent, "install")}
                    >
                      {s?.installed ? "更新接线" : "预览接线"}
                    </button>
                    <button
                      type="button"
                      className="quiet"
                      disabled={busy || !s?.installed}
                      onClick={() => void prepare(agent, "uninstall")}
                    >
                      卸载接线
                    </button>
                  </div>
                </article>
              );
            })}
          </div>
          {pending && (
            <section className="preview-panel">
              <h3>
                {pending.operation === "install" ? "添加工作事件连接" : "移除工作事件连接"} ·{" "}
                {agentNames[pending.agent]}
              </h3>
              <p className="file-path">{pending.target}</p>
              {pending.warnings.map((w) => (
                <p key={w}>{w}</p>
              ))}
              <details>
                <summary>查看接线详情</summary>
                <pre>{pending.changes}</pre>
              </details>
              <div className="actions">
                <button
                  type="button"
                  disabled={busy || !pending.changed}
                  onClick={() => void confirm()}
                >
                  {pending.changed ? "确认写入" : "配置已是最新"}
                </button>
                <button type="button" className="quiet" onClick={() => setPending(null)}>
                  取消
                </button>
              </div>
            </section>
          )}
          <p className="hint">
            提示结束后，她会回到仍在进行的工作。两家同时运行时，需要你处理的事项会优先出现。
          </p>
        </>
      )}
      {tab === "character" && (
        <>
          <div className="section-title">
            <h2>一个角色，不同面貌</h2>
            <p>自动形态会随当前主导的工作来源切换。</p>
          </div>
          <section className="panel">
            <label>
              当前角色
              <select
                value={config.characterId}
                data-testid="character-select"
                onChange={(e) =>
                  setConfig({
                    ...config,
                    characterId: e.target.value,
                    formLock: null,
                    agentForms: {},
                  })
                }
              >
                {characters.length ? (
                  characters.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.name}
                    </option>
                  ))
                ) : (
                  <option value="builtin">{builtinManifest.name}</option>
                )}
              </select>
            </label>
            <label>
              形态选择
              <select
                value={config.formLock ?? ""}
                data-testid="form-select"
                onChange={(e) => setConfig({ ...config, formLock: e.target.value || null })}
              >
                <option value="">自动切换</option>
                {character?.forms.map((f) => (
                  <option key={f.id} value={f.id}>
                    {f.id}
                  </option>
                ))}
              </select>
            </label>
            {!config.formLock &&
              (["claude-code", "codex"] as Agent[]).map((agent) => (
                <label key={agent}>
                  {agentNames[agent]} 形态
                  <select
                    value={config.agentForms[agent] ?? ""}
                    onChange={(e) => {
                      const forms = { ...config.agentForms };
                      if (e.target.value) forms[agent] = e.target.value;
                      else delete forms[agent];
                      setConfig({ ...config, agentForms: forms });
                    }}
                  >
                    <option value="">角色包默认</option>
                    {character?.forms.map((f) => (
                      <option key={f.id} value={f.id}>
                        {f.id}
                      </option>
                    ))}
                  </select>
                </label>
              ))}
            <button type="button" disabled={busy || !isTauri()} onClick={() => void save()}>
              保存角色选择
            </button>
            {character?.license && <p className="hint">{character.license}</p>}
          </section>
          <section className="panel" data-testid="animation-preview-panel">
            <h3>手动状态预览</h3>
            <label className="toggle-row">
              <span>
                <strong>开启手动状态预览</strong>
                <small>选择角色的显示状态，真实工作信息仍会持续更新。</small>
              </span>
              <input
                type="checkbox"
                data-testid="animation-preview-enabled"
                checked={animationPreview?.enabled ?? false}
                disabled={busy || !isTauri() || !animationPreview}
                onChange={(e) => {
                  const enabled = e.target.checked;
                  void task(async () => {
                    acceptAnimationPreview(
                      await invoke<AnimationPreview>("set_animation_preview_enabled", { enabled }),
                    );
                  });
                }}
              />
            </label>
            {animationPreview?.enabled && (
              <div className="animation-preview-controls">
                <label>
                  预览状态
                  <select
                    aria-label="预览动画状态"
                    data-testid="animation-preview-state"
                    value={previewState ?? ""}
                    disabled={busy || !isTauri()}
                    onChange={(e) => {
                      const state = e.target.value as DisplayState;
                      void task(async () => {
                        acceptAnimationPreview(
                          await invoke<AnimationPreview>("set_animation_preview_state", { state }),
                        );
                      });
                    }}
                  >
                    <option value="" disabled>
                      选择预览状态
                    </option>
                    {(
                      [
                        "idle",
                        "thinking",
                        "working",
                        "waiting-permission",
                        "notification",
                        "done",
                        "error",
                        "sleeping",
                      ] as const
                    ).map((state) => (
                      <option key={state} value={state}>
                        {STATE_LABELS[state]}
                      </option>
                    ))}
                  </select>
                </label>
                <p data-testid="animation-preview-mode">
                  当前显示：
                  {previewState
                    ? `手动预览 · ${STATE_LABELS[previewState]}`
                    : `真实状态 · ${STATE_LABELS[p.state]}`}
                </p>
                <p data-testid="animation-preview-real-state">
                  实时工作状态：{STATE_LABELS[p.state]}
                </p>
                {missingPreviewClip && (
                  <p className="notice" data-testid="animation-preview-missing">
                    缺少对应素材，当前显示待机。
                  </p>
                )}
                <button
                  type="button"
                  data-testid="animation-preview-return"
                  disabled={busy || !isTauri() || !previewState}
                  onClick={() =>
                    void task(async () => {
                      acceptAnimationPreview(
                        await invoke<AnimationPreview>("set_animation_preview_state", {
                          state: null,
                        }),
                      );
                    })
                  }
                >
                  返回真实状态
                </button>
                <p className="hint">关闭设置窗口后仍保留手动预览；重启代码娘后自动回到真实状态。</p>
              </div>
            )}
          </section>
          <section className="panel">
            <h3>导入角色</h3>
            <p>
              支持代码娘角色文件夹，以及 Codex v1 / v2 的 PNG、WebP
              图集。导入后保存在代码娘的角色目录中。
            </p>
            <div className="path-input">
              <input
                aria-label="角色来源路径"
                value={source}
                onChange={(e) => setSource(e.target.value)}
                placeholder="选择文件或角色文件夹"
              />
              <button
                type="button"
                disabled={busy || !isTauri()}
                onClick={() => void choose(false)}
              >
                选择图集
              </button>
              <button type="button" disabled={busy || !isTauri()} onClick={() => void choose(true)}>
                选择文件夹
              </button>
            </div>
            <button
              type="button"
              disabled={busy || !source || !isTauri()}
              onClick={() => void importCharacter()}
            >
              导入并使用
            </button>
          </section>
        </>
      )}
      {tab === "preferences" && (
        <>
          <div className="section-title">
            <h2>让她适合你的桌面</h2>
            <p>随时调整出现方式与动画强度。</p>
          </div>
          <section className="panel">
            {(
              [
                ["ghost", "点击穿透", "鼠标直接操作下方窗口；可从托盘切回。"],
                ["reducedMotion", "减少动画", "使用静帧，保留工作状态与提示。"],
                ["fullscreenHide", "全屏时让位", "游戏或演示全屏时自动隐藏。"],
                ["autostart", "开机启动", "登录 Windows 后启动代码娘。"],
                ["userHidden", "暂时隐藏", "可从托盘重新显示。"],
              ] as const
            ).map(([key, title, description]) => (
              <label className="toggle-row" key={key}>
                <span>
                  <strong>{title}</strong>
                  <small>{description}</small>
                </span>
                <input
                  type="checkbox"
                  checked={config[key]}
                  data-testid={`preference-${key}`}
                  onChange={(e) => setConfig({ ...config, [key]: e.target.checked })}
                />
              </label>
            ))}
            <label>
              动画显示方式
              <select
                value={config.renderer}
                data-testid="renderer-select"
                onChange={(e) => setConfig({ ...config, renderer: e.target.value })}
              >
                <option value="pixi">标准</option>
                <option value="dom">兼容模式</option>
              </select>
            </label>
            <button type="button" disabled={busy || !isTauri()} onClick={() => void save()}>
              保存使用偏好
            </button>
          </section>
        </>
      )}
      {tab === "diagnostics" && (
        <>
          <div className="section-title">
            <h2>检查工作联动</h2>
            <p>记录事件名称、状态与短会话标识。提示内容、代码和路径不进入事件记录。</p>
          </div>
          <section className="panel">
            <p>
              当前可信度：
              {p.certainty === "observed"
                ? "有事件依据"
                : p.certainty === "stale"
                  ? "消息较旧"
                  : "暂不确定"}{" "}
              · 丢失或无效事件：{p.missedEvents}
            </p>
            <div className="actions">
              <button
                type="button"
                disabled={busy || !isTauri()}
                onClick={() =>
                  void task(async () =>
                    setDiagnostics(await invoke<Diagnostic[]>("get_diagnostics")),
                  )
                }
              >
                刷新事件记录
              </button>
              <button
                type="button"
                className="quiet"
                disabled={busy || !isTauri()}
                onClick={() =>
                  void task(async () => {
                    await invoke("reset_sessions");
                    setMessage("已清理旧状态，等待新的工作事件");
                  })
                }
              >
                清理旧状态
              </button>
            </div>
            <table>
              <thead>
                <tr>
                  <th>来源</th>
                  <th>会话</th>
                  <th>状态</th>
                  <th>待处理</th>
                </tr>
              </thead>
              <tbody>
                {p.sessions.map((s) => (
                  <tr key={`${s.agent}:${s.label}`}>
                    <td>{agentNames[s.agent]}</td>
                    <td>{s.label}</td>
                    <td>{STATE_LABELS[s.state]}</td>
                    <td>{s.pendingCount}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            <details>
              <summary>事件记录（{diagnostics.length}）</summary>
              <pre>
                {diagnostics
                  .map(
                    (d) =>
                      `${d.seq}  ${d.agent ?? "接收服务"}  ${d.event}  ${d.sessionLabel ?? "—"}  ${d.outcome}`,
                  )
                  .join("\n") || "点击刷新查看最近事件"}
              </pre>
            </details>
          </section>
        </>
      )}
      <footer>本机运行 · 默认无声音 · 无遥测</footer>
    </main>
  );
}
const host = document.getElementById("app");
if (host) {
  createRoot(host).render(
    <StrictMode>
      <Settings />
    </StrictMode>,
  );
}

import "virtual:desktop-test";
