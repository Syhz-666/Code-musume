import {
  type AnimationPreview,
  AnimationPreviewStore,
  DEFAULT_ANIMATION_PREVIEW,
  type Projection,
  ProjectionStore,
} from "@code-musume/protocol";
import { Channel, invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export { invoke, isTauri };
export async function watchAnimationPreview(
  callback: (p: AnimationPreview) => void,
): Promise<() => void> {
  if (!isTauri()) {
    callback({ ...DEFAULT_ANIMATION_PREVIEW });
    return () => {};
  }
  const store = new AnimationPreviewStore();
  let stopped = false;
  const accept = (p: AnimationPreview) => {
    if (!stopped && store.accept(p)) callback(p);
  };
  const unlisten = await listen<AnimationPreview>("animation-preview-changed", (e) =>
    accept(e.payload),
  );
  try {
    accept(await invoke<AnimationPreview>("get_animation_preview"));
  } catch (error) {
    stopped = true;
    unlisten();
    throw error;
  }
  return () => {
    stopped = true;
    unlisten();
  };
}
export const preview: Projection = {
  v: 2,
  instanceId: "preview",
  revision: 0,
  state: "idle",
  certainty: "unknown",
  dominantAgent: null,
  sessionLabel: null,
  pendingCount: 0,
  activeSessions: 0,
  formId: "default",
  characterId: "builtin",
  bubbleKey: null,
  intensity: 0.25,
  missedEvents: 0,
  sessions: [],
  capabilities: [],
};
export async function watchProjection(callback: (p: Projection) => void): Promise<() => void> {
  if (!isTauri()) {
    callback(preview);
    return () => {};
  }
  const store = new ProjectionStore();
  const id = crypto.randomUUID();
  const channel = new Channel<Projection>();
  let stopped = false;
  const accept = (p: Projection) => {
    if (!stopped && store.accept(p)) callback(p);
  };
  channel.onmessage = accept;
  accept(
    await invoke<Projection>("subscribe_projection", { onUpdate: channel, subscriptionId: id }),
  );
  return () => {
    stopped = true;
    void invoke("unsubscribe_projection", { subscriptionId: id });
  };
}
