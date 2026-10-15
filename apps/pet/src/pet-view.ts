import {
  type CharacterForm,
  type CharacterManifest,
  type Config,
  type Projection,
  STATE_LABELS,
} from "@code-musume/protocol";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Application, Container, Graphics, Rectangle, Sprite, Texture } from "pixi.js";
import { invoke, isTauri, preview, watchProjection } from "./bridge";
import "./pet.css";
import "virtual:desktop-test";

const defaults: Config = {
  schemaVersion: 1,
  characterId: "builtin",
  formLock: null,
  agentForms: {},
  ghost: false,
  userHidden: false,
  reducedMotion: false,
  fullscreenHide: true,
  autostart: false,
  renderer: "pixi",
  position: null,
};
let config = { ...defaults },
  current = preview;
let manifest: CharacterManifest | null = null,
  form: CharacterForm | null = null;
let elapsed = 0,
  touchUntil = 0,
  lastTouch = -5000,
  loadEpoch = 0,
  visible = true;
let renderFrames = 0,
  renderSince = performance.now();
let characterReady = false;
let receiptScheduled = false;
let acknowledged = "";
const motion = matchMedia("(prefers-reduced-motion: reduce)");
const host = document.getElementById("app");
if (!host) throw new Error("Missing pet host");
host.innerHTML =
  '<div class="bubble" aria-live="polite"></div><div class="character" role="button" aria-label="拖动代码娘"></div><div class="agent-label"></div><button class="exit-button" type="button" aria-label="退出代码娘" title="退出代码娘">退出</button><button class="settings-button" type="button" aria-label="打开设置">⚙</button>';
const bubble = host.querySelector<HTMLDivElement>(".bubble"),
  stage = host.querySelector<HTMLDivElement>(".character"),
  label = host.querySelector<HTMLDivElement>(".agent-label");
if (!bubble || !stage || !label) throw new Error("Missing pet view");
const bubbleNode = bubble,
  stageNode = stage,
  labelNode = label;
let app: Application | null = null,
  blob: Container | null = null,
  body: Graphics | null = null,
  eyes: Container | null = null,
  hand: Graphics | null = null,
  sprite: Sprite | null = null,
  texture: Texture | null = null;
let frames: Texture[] = [],
  frameKey = "",
  domFrame = -1,
  actionSince = 0,
  lastAction = "";
const colors: Record<string, number> = { default: 0xb9dce7, claude: 0xf3c6ac, codex: 0xc9c5fa };
function reduced() {
  return config.reducedMotion || motion.matches;
}
function color() {
  return colors[current.formId] ?? 0xb9dce7;
}
function companion() {
  if (current.state === "idle" && elapsed - lastTouch >= 5000 && !reduced()) {
    lastTouch = elapsed;
    touchUntil = elapsed + 850;
  }
}
stageNode.addEventListener("pointerdown", (e) => {
  if (e.button === 0) {
    companion();
    if (isTauri())
      void getCurrentWindow()
        .startDragging()
        .catch(() => {});
  }
});
host.querySelector(".settings-button")?.addEventListener("click", () => {
  if (isTauri()) void invoke("show_settings");
});
const exitButton = host.querySelector<HTMLButtonElement>(".exit-button");
if (exitButton) {
  exitButton.disabled = !isTauri();
  exitButton.addEventListener("click", () => {
    exitButton.disabled = true;
    void invoke("quit_app").catch(() => {
      exitButton.disabled = false;
      bubbleNode.textContent = "退出失败，请从托盘重试";
      bubbleNode.classList.add("shown");
    });
  });
}
document.addEventListener("contextmenu", (e) => e.preventDefault());
document.addEventListener(
  "wheel",
  (e) => {
    if (e.ctrlKey) e.preventDefault();
  },
  { passive: false },
);
async function initRenderer() {
  if (config.renderer === "dom") {
    stageNode.innerHTML =
      '<div class="dom-pet"><div class="ear left"></div><div class="ear right"></div><div class="face"><i></i><i></i><span>ω</span></div><div class="bow">✦</div></div><div class="dom-sprite"></div>';
    return;
  }
  app = new Application();
  await app.init({
    backgroundAlpha: 0,
    width: 160,
    height: 166,
    antialias: true,
    resolution: Math.min(devicePixelRatio || 1, 2),
    autoDensity: true,
  });
  stageNode.appendChild(app.canvas);
  blob = new Container();
  blob.position.set(80, 85);
  body = new Graphics()
    .ellipse(-33, -38, 15, 24)
    .fill(0xffffff)
    .ellipse(33, -38, 15, 24)
    .fill(0xffffff)
    .circle(0, 0, 48)
    .fill(0xffffff)
    .stroke({ color: 0x546174, width: 2 });
  blob.addChild(body);
  hand = new Graphics().ellipse(47, 6, 11, 19).fill(0xffffff);
  hand.pivot.set(43, 0);
  hand.position.set(43, 0);
  blob.addChild(hand);
  eyes = new Container();
  eyes.y = -4;
  eyes.addChild(
    new Graphics().ellipse(-17, 0, 4, 7).fill(0x303447),
    new Graphics().ellipse(17, 0, 4, 7).fill(0x303447),
  );
  blob.addChild(eyes);
  blob.addChild(
    new Graphics()
      .circle(-27, 11, 5)
      .fill({ color: 0xf6a4ae, alpha: 0.7 })
      .circle(27, 11, 5)
      .fill({ color: 0xf6a4ae, alpha: 0.7 })
      .moveTo(-7, 17)
      .quadraticCurveTo(-3, 22, 0, 17)
      .quadraticCurveTo(3, 22, 7, 17)
      .stroke({ color: 0x655067, width: 1.8 }),
  );
  app.stage.addChild(blob);
  sprite = new Sprite();
  sprite.anchor.set(0.5);
  sprite.position.set(80, 83);
  sprite.visible = false;
  app.stage.addChild(sprite);
  app.ticker.maxFPS = 24;
  app.ticker.add((ticker) => draw(ticker.deltaMS));
}
function clearFrames() {
  for (const t of frames) t.destroy(false);
  frames = [];
  frameKey = "";
  domFrame = -1;
}
async function loadCharacter(p: Projection) {
  characterReady = false;
  const epoch = ++loadEpoch;
  let m = manifest;
  if (m?.id !== p.characterId && isTauri()) {
    try {
      m = await invoke<CharacterManifest>("get_character", { id: p.characterId });
    } catch {
      m = null;
    }
  }
  if (epoch !== loadEpoch) return;
  manifest = m;
  form =
    m?.forms.find((f) => f.id === p.formId) ??
    m?.forms.find((f) => f.id === m?.defaultForm) ??
    null;
  if (sprite) sprite.visible = false;
  clearFrames();
  if (texture) {
    texture.destroy(true);
    texture = null;
  }
  if (form?.image && isTauri()) {
    try {
      const source = await invoke<string>("get_character_asset", {
        id: p.characterId,
        form: form.id,
      });
      if (epoch !== loadEpoch) return;
      const image = new Image();
      image.src = source;
      await image.decode();
      if (epoch !== loadEpoch) return;
      if (config.renderer === "dom") {
        const el = stageNode.querySelector<HTMLElement>(".dom-sprite");
        if (el) {
          el.style.backgroundImage = `url("${source}")`;
          el.style.display = "block";
        }
      } else {
        texture = Texture.from(image);
      }
    } catch {
      if (epoch !== loadEpoch) return;
      form = null;
    }
  }
  const builtin = !form?.image || (!texture && config.renderer !== "dom");
  if (blob) blob.visible = builtin;
  if (sprite) sprite.visible = !builtin;
  const el = stageNode.querySelector<HTMLElement>(".dom-pet"),
    ds = stageNode.querySelector<HTMLElement>(".dom-sprite");
  if (el) el.style.display = form?.image ? "none" : "block";
  if (ds && !form?.image) ds.style.display = "none";
  characterReady = true;
  submitReceipt();
}
function submitReceipt() {
  if (!isTauri() || !visible || !characterReady || receiptScheduled) return;
  const key = `${current.instanceId}:${current.revision}:${loadEpoch}`;
  if (key === acknowledged) return;
  receiptScheduled = true;
  requestAnimationFrame(() => {
    draw(0);
    app?.render();
    // A second frame callback follows submission of both canvas and DOM updates.
    requestAnimationFrame(() => {
      receiptScheduled = false;
      if (
        !visible ||
        !characterReady ||
        key !== `${current.instanceId}:${current.revision}:${loadEpoch}`
      ) {
        submitReceipt();
        return;
      }
      acknowledged = key;
      hostNode.dataset.renderedRevision = String(current.revision);
      void invoke("report_rendered", {
        instanceId: current.instanceId,
        revision: current.revision,
      }).catch(() => {});
    });
  });
}
const hostNode = host;
function apply(p: Projection) {
  const changed = p.characterId !== current.characterId || p.formId !== current.formId;
  if (p.state !== current.state) {
    lastAction = "";
    actionSince = elapsed;
  }
  current = p;
  Object.assign(hostNode.dataset, {
    state: p.state,
    agent: p.dominantAgent ?? "",
    form: p.formId,
    certainty: p.certainty,
    pending: String(p.pendingCount),
    revision: String(p.revision),
    instance: p.instanceId,
    renderer: config.renderer,
    character: p.characterId,
  });
  labelNode.textContent =
    p.dominantAgent === "claude-code"
      ? "CLAUDE"
      : p.dominantAgent === "codex"
        ? "CODEX"
        : isTauri()
          ? "代码娘"
          : "界面预览";
  bubbleNode.textContent =
    p.bubbleKey === "uncertain" ? "暂时没有新消息" : p.bubbleKey ? STATE_LABELS[p.state] : "";
  bubbleNode.classList.toggle("shown", !!p.bubbleKey);
  stageNode.dataset.state = p.state;
  if (body) body.tint = color();
  if (hand) hand.tint = color();
  stageNode.style.setProperty("--pet-color", `#${color().toString(16).padStart(6, "0")}`);
  if (app)
    app.ticker.maxFPS = reduced() || p.state === "sleeping" ? 1 : p.state === "idle" ? 24 : 60;
  if (changed) void loadCharacter(p);
  else submitReceipt();
}
function draw(delta: number) {
  if (!visible) return;
  elapsed += Math.min(delta, 1000);
  const animate = !reduced();
  renderFrames++;
  const renderNow = performance.now();
  if (renderNow - renderSince >= 2000) {
    if (isTauri())
      void invoke("report_render_stats", {
        fps: (renderFrames * 1000) / (renderNow - renderSince),
      }).catch(() => {});
    renderFrames = 0;
    renderSince = renderNow;
  }
  const jump =
    animate && elapsed < touchUntil && current.state === "idle"
      ? Math.sin(((touchUntil - elapsed) / 850) * Math.PI) * 15
      : 0;
  if (blob && eyes && hand) {
    const active = current.state === "working";
    blob.y =
      85 + (animate ? Math.sin(elapsed / (active ? 250 : 1300)) * (active ? 4 : 2.5) : 0) - jump;
    blob.rotation = animate && current.state === "error" ? Math.sin(elapsed / 90) * 0.035 : 0;
    eyes.scale.y = current.state === "sleeping" || (animate && elapsed % 4200 > 4020) ? 0.15 : 1;
    eyes.x = animate && current.state === "thinking" ? Math.sin(elapsed / 1400) * 3 : 0;
    hand.rotation =
      animate && (current.state === "waiting-permission" || current.state === "done")
        ? Math.sin(elapsed / 280) * 0.45 * current.intensity
        : 0;
  }
  const pet = stageNode.querySelector<HTMLElement>(".dom-pet");
  if (pet)
    pet.style.transform = `translateY(${animate ? Math.sin(elapsed / 1300) * 2 - jump : 0}px)`;
  if (!form?.image) return;
  const action =
      animate && elapsed < touchUntil && current.state === "idle" ? "touch" : current.state,
    clip = form.clips[action] ?? form.clips.idle;
  if (!clip) return;
  if (action !== lastAction) {
    actionSince = elapsed;
    lastAction = action;
  }
  const tick = Math.floor(((elapsed - actionSince) / 1000) * clip.fps),
    index = !animate ? 0 : clip.looping ? tick % clip.frames : Math.min(clip.frames - 1, tick);
  if (config.renderer === "dom") {
    const el = stageNode.querySelector<HTMLElement>(".dom-sprite");
    if (el && domFrame !== clip.row * form.columns + index) {
      domFrame = clip.row * form.columns + index;
      const scale = 144 / form.cellWidth;
      el.style.width = "144px";
      el.style.height = `${form.cellHeight * scale}px`;
      el.style.backgroundSize = `${form.cellWidth * form.columns * scale}px auto`;
      el.style.backgroundPosition = `${-index * 144}px ${-clip.row * form.cellHeight * scale}px`;
    }
  } else if (sprite && texture) {
    const key = `${clip.row}:${clip.frames}`;
    if (frameKey !== key) {
      clearFrames();
      frameKey = key;
      for (let i = 0; i < clip.frames; i++)
        frames.push(
          new Texture({
            source: texture.source,
            frame: new Rectangle(
              i * form.cellWidth,
              clip.row * form.cellHeight,
              form.cellWidth,
              form.cellHeight,
            ),
          }),
        );
    }
    const frame = frames[index];
    if (frame) {
      sprite.texture = frame;
      sprite.width = 144;
      sprite.height = (144 * form.cellHeight) / form.cellWidth;
    }
  }
}
async function main() {
  if (isTauri()) config = await invoke<Config>("get_config");
  await initRenderer();
  let stopped = false,
    last = performance.now();
  const domTick = () => {
    if (stopped) return;
    const now = performance.now();
    if (visible) draw(now - last);
    last = now;
    const fps =
      !visible || reduced() || current.state === "sleeping"
        ? 1
        : current.state === "idle"
          ? 24
          : 60;
    setTimeout(domTick, 1000 / fps);
  };
  if (config.renderer === "dom") domTick();
  const stop = await watchProjection(apply);
  await loadCharacter(current);
  apply(current);
  if (isTauri()) {
    await listen<boolean>("pet-visibility", async (e) => {
      visible = e.payload;
      renderFrames = 0;
      renderSince = performance.now();
      const next = await invoke<Config>("get_config");
      if (next.renderer !== config.renderer) {
        location.reload();
        return;
      }
      config = next;
      if (visible) {
        apply(await invoke<Projection>("get_projection"));
        app?.start();
      } else app?.stop();
    });
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
    );
    await invoke("pet_ready");
  }
  window.addEventListener(
    "pagehide",
    () => {
      stopped = true;
      stop();
      app?.destroy(true);
    },
    { once: true },
  );
  motion.addEventListener("change", () => apply(current));
}
void main().catch((error: unknown) => {
  bubbleNode.textContent = "暂时无法连接，打开设置检查";
  bubbleNode.classList.add("shown");
  console.error("pet initialization failed", error);
});
