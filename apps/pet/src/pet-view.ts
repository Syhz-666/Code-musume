import type { BanterProjection } from "@code-musume/protocol";
import {
  type AnimationPreview,
  type CharacterForm,
  type CharacterManifest,
  type Clip,
  type Config,
  DEFAULT_ANIMATION_PREVIEW,
  type Projection,
  STATE_LABELS,
} from "@code-musume/protocol";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Application, Container, Graphics, Rectangle, Sprite, Texture } from "pixi.js";
import {
  invoke,
  isTauri,
  preview,
  watchAnimationPreview,
  watchBanter,
  watchProjection,
} from "./bridge";
import { builtinAssetUrl, builtinManifest } from "./builtin-character";
import { spriteFramePosition } from "./sprite-layout";
import { SpritePlayback } from "./sprite-playback";
import { SpriteTransition } from "./sprite-transition";
import { CostumePlayback, WaitingPresentation } from "./waiting-presentation";
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
  manualAnimationEnabled: false,
  fullscreenHide: true,
  autostart: false,
  renderer: "pixi",
  position: null,
};
let config = { ...defaults },
  current = preview;
let animationPreview: AnimationPreview = { ...DEFAULT_ANIMATION_PREVIEW };
const waitingPresentation = new WaitingPresentation();
let visualTarget = {
  characterId: current.characterId,
  formId: current.formId,
  state: current.state,
};
function manualPreviewActive() {
  return animationPreview.enabled && animationPreview.state !== null;
}
function displayedState() {
  return (animationPreview.enabled ? animationPreview.state : null) ?? current.state;
}
function displayFPS() {
  if (!visible || reduced()) return 1;
  const state = displayedState();
  if (
    visualTarget.state !== state ||
    loadedForm !== formKey(current) ||
    costumeChange ||
    playbackMoving ||
    transition.active
  )
    return 60;
  if (state === "sleeping") {
    const clip = form?.clips.sleeping;
    return clip && clip.frames > 1 ? clip.fps : 1;
  }
  return state === "idle" || state === "thinking" ? 24 : 60;
}
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
const atlases = new Map<
  string,
  { image: HTMLImageElement; texture: Texture | null; url: string }
>();
let loadedCharacter: string | null = null;
let loadedForm: string | null = null;
let loadingForm: string | null = null;
let switchScheduled = false;
let costumeChange: {
  from: CharacterForm;
  key: string;
  clip: Clip;
  playback: CostumePlayback;
} | null = null;
let characterLoad: Promise<void> | null = null;
let wakeDOM = () => {};
let receiptScheduled = false;
let acknowledged = "";
const motion = matchMedia("(prefers-reduced-motion: reduce)");
const host = document.getElementById("app");
if (!host) throw new Error("Missing pet host");
host.innerHTML =
  '<div class="bubble" aria-live="polite"></div><div class="character" role="button" aria-label="拖动代码娘" title="右键打开菜单"></div><div class="agent-label"></div>';
const bubble = host.querySelector<HTMLDivElement>(".bubble"),
  stage = host.querySelector<HTMLDivElement>(".character"),
  label = host.querySelector<HTMLDivElement>(".agent-label");
if (!bubble || !stage || !label) throw new Error("Missing pet view");
const bubbleNode = bubble,
  stageNode = stage,
  labelNode = label;
const playback = new SpritePlayback();
const transition = new SpriteTransition(stageNode);
let playbackMoving = false;
let poseReady = false;
let app: Application | null = null,
  blob: Container | null = null,
  body: Graphics | null = null,
  eyes: Container | null = null,
  hand: Graphics | null = null,
  sprite: Sprite | null = null;
let frames: Texture[] = [],
  frameKey = "",
  domFrame = "";
const colors: Record<string, number> = { default: 0xb9dce7, claude: 0xf3c6ac, codex: 0xc9c5fa };
function reduced() {
  return config.reducedMotion || motion.matches;
}
function color() {
  return colors[current.formId] ?? 0xb9dce7;
}
function companion() {
  elapsed = performance.now();
  if (
    !manualPreviewActive() &&
    current.state === "idle" &&
    elapsed - lastTouch >= 5000 &&
    !reduced()
  ) {
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
let contextMenuOpen = false;
document.addEventListener("contextmenu", (e) => {
  e.preventDefault();
  if (!isTauri() || contextMenuOpen) return;
  contextMenuOpen = true;
  void invoke("show_pet_menu", { x: e.clientX, y: e.clientY })
    .catch(() => {
      bubbleNode.textContent = "菜单打开失败，请从托盘操作";
      bubbleNode.classList.add("shown");
    })
    .finally(() => {
      contextMenuOpen = false;
    });
});
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
    stageNode.appendChild(transition.canvas);
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
  app.ticker.add(() => draw());
}
function clearFrames() {
  for (const t of frames) t.destroy(false);
  frames = [];
  frameKey = "";
  domFrame = "";
}
function formKey(p: Pick<Projection, "characterId" | "formId">) {
  return `${p.characterId}:${p.formId}`;
}
function formPending() {
  return loadedForm !== formKey(visualTarget);
}
function costumeClip() {
  return (
    loadedCharacter === visualTarget.characterId && form?.clips[`form-to-${visualTarget.formId}`]
  );
}
function refreshVisualTarget(now: number, immediate = false) {
  const previous = visualTarget;
  visualTarget = waitingPresentation.resolve(
    { characterId: current.characterId, formId: current.formId, state: displayedState() },
    now,
    immediate || reduced() || manualPreviewActive(),
  );
  return formKey(previous) !== formKey(visualTarget);
}
function syncCharacter(immediate: boolean) {
  const key = formKey(visualTarget);
  if (loadingForm === key || (!loadingForm && !formPending())) return;
  if (
    !immediate &&
    visible &&
    !reduced() &&
    loadedCharacter === visualTarget.characterId &&
    (costumeChange || (characterReady && costumeClip()))
  )
    return;
  characterLoad = loadCharacter(visualTarget);
}
async function loadCharacter(p: Pick<Projection, "characterId" | "formId">, playCostume = false) {
  const previous = form;
  const canPlayChange = playCostume && characterReady && loadedCharacter === p.characterId;
  costumeChange = null;
  characterReady = false;
  loadingForm = formKey(p);
  const epoch = ++loadEpoch;
  let m = manifest;
  if (m?.id !== p.characterId) {
    try {
      m = isTauri()
        ? await invoke<CharacterManifest>("get_character", { id: p.characterId })
        : p.characterId === "builtin"
          ? builtinManifest
          : null;
    } catch {
      m = null;
    }
  }
  if (epoch !== loadEpoch) return;
  manifest = m;
  const target =
    m?.forms.find((f) => f.id === p.formId) ??
    m?.forms.find((f) => f.id === m?.defaultForm) ??
    null;
  let selectedForm = target;
  const changeKey = `form-to-${target?.id ?? ""}`;
  const changeClip = canPlayChange && previous?.id !== target?.id && previous?.clips[changeKey];
  const names = target
    ? new Set([
        target.image,
        ...Object.values(target.clips).map((clip) => clip.image ?? target.image),
      ])
    : new Set<string>();
  const reuseAssets = [...names].every((name) => atlases.has(`${p.characterId}:${name}`));
  if ((!reuseAssets && !changeClip) || loadedCharacter !== p.characterId) {
    if (sprite) sprite.visible = false;
    transition.reset();
    playback.reset();
    poseReady = false;
  }
  if (loadedCharacter !== p.characterId) {
    for (const atlas of atlases.values()) atlas.texture?.destroy(true);
    atlases.clear();
    loadedCharacter = p.characterId;
  }
  if (target?.image && !reuseAssets) {
    try {
      await Promise.all(
        [...names].map(async (name) => {
          const key = `${p.characterId}:${name}`;
          if (atlases.has(key)) return;
          const source = isTauri()
            ? await invoke<string>("get_character_asset", {
                id: p.characterId,
                form: target.id,
                image: name,
              })
            : builtinAssetUrl(name);
          if (epoch !== loadEpoch) return;
          const image = new Image();
          image.src = source;
          await image.decode();
          if (epoch !== loadEpoch) return;
          atlases.set(key, {
            image,
            url: source,
            texture: config.renderer === "dom" ? null : Texture.from(image),
          });
        }),
      );
    } catch {
      if (epoch !== loadEpoch) return;
      selectedForm = null;
    }
  }
  if (epoch !== loadEpoch) return;
  clearFrames();
  loadedForm = formKey(p);
  loadingForm = null;
  if (previous?.id !== selectedForm?.id) {
    playback.reset();
    poseReady = false;
  }
  form = selectedForm;
  if (previous && changeClip && form && visible && !reduced()) {
    costumeChange = {
      from: previous,
      key: changeKey,
      clip: changeClip,
      playback: new CostumePlayback(performance.now(), changeClip.frames, changeClip.fps),
    };
  }
  const builtin = !form?.image || !atlases.has(`${p.characterId}:${form.image}`);
  if (blob) blob.visible = builtin;
  if (sprite) sprite.visible = !builtin;
  const el = stageNode.querySelector<HTMLElement>(".dom-pet"),
    ds = stageNode.querySelector<HTMLElement>(".dom-sprite");
  if (el) el.style.display = form?.image ? "none" : "block";
  if (ds && !form?.image) ds.style.display = "none";
  characterReady = true;
  draw();
  app?.render();
  wakeDOM();
  submitReceipt();
}
function visualReady() {
  return (
    loadedForm === formKey(current) &&
    visualTarget.state === displayedState() &&
    (!form?.image || (poseReady && !costumeChange && !transition.active))
  );
}
function submitReceipt() {
  if (
    !isTauri() ||
    !visible ||
    !characterReady ||
    !visualReady() ||
    receiptScheduled ||
    manualPreviewActive()
  )
    return;
  const receiptKey = () =>
    `${current.instanceId}:${current.revision}:${loadEpoch}:${animationPreview.revision}`;
  const key = receiptKey();
  if (key === acknowledged) return;
  receiptScheduled = true;
  requestAnimationFrame(() => {
    if (manualPreviewActive() || key !== receiptKey()) {
      receiptScheduled = false;
      submitReceipt();
      return;
    }
    draw();
    app?.render();
    // A second frame callback follows submission of both canvas and DOM updates.
    requestAnimationFrame(() => {
      receiptScheduled = false;
      if (
        !visible ||
        !characterReady ||
        !visualReady() ||
        manualPreviewActive() ||
        key !== receiptKey()
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
let banter: BanterProjection | undefined;
function updateBubble() {
  const p = current;
  const summary =
    banter?.instanceId === p.instanceId &&
    !manualPreviewActive() &&
    (banter.kind === "completion"
      ? p.state === "idle" && !p.bubbleKey
      : banter.kind === "progress" &&
        banter.agent === p.dominantAgent &&
        banter.sessionLabel === p.sessionLabel &&
        p.certainty !== "unknown" &&
        (p.state === "working" || p.state === "thinking"))
      ? banter.text
      : null;
  const status =
    p.bubbleKey === "uncertain" ? "暂时没有新消息" : p.bubbleKey ? STATE_LABELS[p.state] : "";
  const text = status || summary || "";
  bubbleNode.textContent = text;
  bubbleNode.dataset.kind = status ? "status" : summary ? "banter" : "";
  bubbleNode.classList.toggle("shown", Boolean(text));
}
function apply(p: Projection, immediate = false) {
  current = p;
  if (immediate || reduced()) {
    costumeChange = null;
    transition.reset();
    if (immediate) playback.reset();
  }
  Object.assign(hostNode.dataset, {
    state: p.state,
    displayState: displayedState(),
    animationMode: manualPreviewActive() ? "manual" : "real",
    previewRevision: String(animationPreview.revision),
    agent: p.dominantAgent ?? "",
    form: p.formId,
    certainty: p.certainty,
    pending: String(p.pendingCount),
    revision: String(p.revision),
    instance: p.instanceId,
    renderer: config.renderer,
    character: p.characterId,
  });
  labelNode.textContent = manualPreviewActive()
    ? "手动预览"
    : p.dominantAgent === "claude-code"
      ? "CLAUDE"
      : p.dominantAgent === "codex"
        ? "CODEX"
        : isTauri()
          ? "代码娘"
          : "界面预览";
  updateBubble();
  stageNode.dataset.state = displayedState();
  if (body) body.tint = color();
  if (hand) hand.tint = color();
  stageNode.style.setProperty("--pet-color", `#${color().toString(16).padStart(6, "0")}`);
  if (app) app.ticker.maxFPS = displayFPS();
  refreshVisualTarget(performance.now(), immediate);
  syncCharacter(immediate);
  draw();
  app?.render();
  submitReceipt();
  wakeDOM();
}
function applyAnimationPreview(next: AnimationPreview) {
  animationPreview = next;
  touchUntil = 0;
  apply(current);
}
function draw() {
  if (!visible) return;
  // Event-driven draws use the event's own time. A long sleeping-frame interval
  // must not be charged to the first frame of a newly requested animation.
  elapsed = performance.now();
  if (refreshVisualTarget(elapsed)) syncCharacter(false);
  const animate = !reduced();
  const state = visualTarget.state;
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
    animate && !manualPreviewActive() && elapsed < touchUntil && state === "idle"
      ? Math.sin(((touchUntil - elapsed) / 850) * Math.PI) * 15
      : 0;
  if (blob && eyes && hand) {
    const active = state === "working";
    blob.y =
      85 + (animate ? Math.sin(elapsed / (active ? 250 : 1300)) * (active ? 4 : 2.5) : 0) - jump;
    blob.rotation = animate && state === "error" ? Math.sin(elapsed / 90) * 0.035 : 0;
    eyes.scale.y = state === "sleeping" || (animate && elapsed % 4200 > 4020) ? 0.15 : 1;
    eyes.x = animate && state === "thinking" ? Math.sin(elapsed / 1400) * 3 : 0;
    hand.rotation =
      animate && (state === "waiting-permission" || state === "done")
        ? Math.sin(elapsed / 280) * 0.45 * (manualPreviewActive() ? 1 : current.intensity)
        : 0;
  }
  const pet = stageNode.querySelector<HTMLElement>(".dom-pet");
  if (pet)
    pet.style.transform = `translateY(${animate ? Math.sin(elapsed / 1300) * 2 - jump : 0}px)`;
  if (!form?.image || !characterReady) return;
  const action =
    animate && !manualPreviewActive() && elapsed < touchUntil && state === "idle" ? "touch" : state;
  const returning = costumeChange?.from.id === visualTarget.formId;
  const changeSample = costumeChange?.playback.sample(elapsed, returning);
  if (costumeChange && (!animate || changeSample?.done)) {
    if (returning) {
      form = costumeChange.from;
      loadedForm = formKey(visualTarget);
      playback.reset();
    }
    costumeChange = null;
    syncCharacter(!animate);
    if (!characterReady) return;
  }
  const preparingCostume = animate && formPending() && costumeClip();
  const sequenced =
    loadedCharacter === "builtin" && (form.id === "codex" || Boolean(preparingCostume));
  const paintedForm = costumeChange?.from ?? form;
  const sample =
    costumeChange && changeSample
      ? {
          clip: costumeChange.clip,
          index: changeSample.index,
          key: costumeChange.key,
          phase: "costume",
          state,
          token: costumeChange.key,
          ready: false,
          transitioning: true,
        }
      : playback.sample(
          form.clips,
          preparingCostume ? "idle" : action,
          elapsed,
          animate,
          sequenced,
        );
  if (!sample) return;
  const { clip } = sample;
  const atNeutral = Boolean(
    !costumeChange && preparingCostume && sample.key === "idle" && sample.ready,
  );
  const index = atNeutral ? 0 : sample.index;
  const imageName = clip.image ?? paintedForm.image;
  const atlas = atlases.get(`${current.characterId}:${imageName}`);
  if (!atlas) return;
  transition.paint(
    atlas.image,
    paintedForm,
    clip,
    index,
    `${current.characterId}:${imageName}:${paintedForm.cellWidth}:${paintedForm.cellHeight}:${paintedForm.columns}:${clip.row}:${clip.column}:${clip.frames}:${clip.fps}:${sample.token}`,
    elapsed,
    animate,
    config.renderer,
    sequenced || costumeChange ? 80 : 180,
  );
  poseReady = sample.ready;
  playbackMoving = sample.transitioning;
  const position = spriteFramePosition(paintedForm.columns, clip, index);
  Object.assign(hostNode.dataset, {
    clip: action,
    frame: String(index),
    frameCount: String(clip.frames),
    spriteColumn: String(position.column),
    spriteRow: String(position.row),
    animationRunning: String(animate && clip.frames > 1),
    systemReducedMotion: String(motion.matches),
    configuredReducedMotion: String(config.reducedMotion),
    segment: sample.key,
    animationPhase: sample.phase,
    visualState: sample.state,
    visualForm: paintedForm.id,
    visualReady: String(visualReady()),
    blending: String(transition.active),
    atlas: imageName,
  });
  if (config.renderer === "dom") {
    const el = stageNode.querySelector<HTMLElement>(".dom-sprite");
    const key = `${imageName}:${position.row}:${position.column}`;
    if (el) {
      el.style.display = "block";
      el.style.visibility = transition.active ? "hidden" : "visible";
    }
    if (el && domFrame !== key) {
      domFrame = key;
      el.style.backgroundImage = `url("${atlas.url}")`;
      const scale = 144 / paintedForm.cellWidth;
      el.style.width = "144px";
      el.style.height = `${paintedForm.cellHeight * scale}px`;
      el.style.backgroundSize = `${paintedForm.cellWidth * paintedForm.columns * scale}px auto`;
      el.style.backgroundPosition = `${-position.column * 144}px ${-position.row * paintedForm.cellHeight * scale}px`;
    }
  } else if (sprite && atlas.texture) {
    const key = `${imageName}:${clip.row}:${clip.column}:${clip.frames}`;
    if (frameKey !== key) {
      clearFrames();
      frameKey = key;
      for (let i = 0; i < clip.frames; i++) {
        const position = spriteFramePosition(paintedForm.columns, clip, i);
        frames.push(
          new Texture({
            source: atlas.texture.source,
            frame: new Rectangle(
              position.column * paintedForm.cellWidth,
              position.row * paintedForm.cellHeight,
              paintedForm.cellWidth,
              paintedForm.cellHeight,
            ),
          }),
        );
      }
    }
    const frame = frames[index];
    if (frame) {
      sprite.visible = !transition.active;
      sprite.texture = frame;
      sprite.width = 144;
      sprite.height = (144 * paintedForm.cellHeight) / paintedForm.cellWidth;
    }
  }
  if (costumeChange) costumeChange.playback.markPresented(index);
  else playback.markPresented({ token: sample.token, index });
  if (atNeutral && !transition.active && !switchScheduled) {
    const target = formKey(visualTarget);
    switchScheduled = true;
    requestAnimationFrame(() => {
      switchScheduled = false;
      if (
        visible &&
        characterReady &&
        !loadingForm &&
        !reduced() &&
        target === formKey(visualTarget) &&
        formPending() &&
        costumeClip()
      ) {
        characterLoad = loadCharacter(visualTarget, true);
      }
    });
  }
  if (import.meta.env.VITE_E2E === "1") {
    // Synchronous evidence cannot coalesce multiple draws like a MutationObserver.
    // Both the fields and event are removed from production builds.
    Object.assign(hostNode.dataset, {
      frameSampledAtMs: String(elapsed),
      frameFps: String(clip.fps),
    });
    window.dispatchEvent(
      new CustomEvent("code-musume:e2e-frame", {
        detail: {
          frame: index,
          frames: clip.frames,
          fps: clip.fps,
          atMs: elapsed,
          state,
          segment: sample.key,
          phase: sample.phase,
          ready: visualReady(),
        },
      }),
    );
  }
  if (app) app.ticker.maxFPS = displayFPS();
  submitReceipt();
}
async function main() {
  if (isTauri()) config = await invoke<Config>("get_config");
  await initRenderer();
  let stopped = false,
    domTimer: ReturnType<typeof setTimeout> | undefined;
  const domTick = () => {
    if (stopped) return;
    if (visible) draw();
    domTimer = setTimeout(domTick, 1000 / displayFPS());
  };
  if (config.renderer === "dom") {
    wakeDOM = () => {
      clearTimeout(domTimer);
      domTimer = setTimeout(domTick, 0);
    };
    domTick();
  }
  const stopAnimationPreview = await watchAnimationPreview(applyAnimationPreview);
  const stop = await watchProjection(apply);
  const stopBanter = await watchBanter((snapshot) => {
    banter = snapshot;
    updateBubble();
  }).catch(() => () => {});
  if (characterLoad) await characterLoad;
  else await loadCharacter(visualTarget);
  apply(current);
  if (isTauri()) {
    await listen<boolean>("pet-visibility", async (e) => {
      const wasVisible = visible;
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
        apply(await invoke<Projection>("get_projection"), !wasVisible);
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
      clearTimeout(domTimer);
      wakeDOM = () => {};
      stop();
      stopAnimationPreview();
      stopBanter();
      app?.destroy(true);
      for (const atlas of atlases.values()) atlas.texture?.destroy(true);
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
