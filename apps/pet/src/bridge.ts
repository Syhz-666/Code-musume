import {
  type AnimationPreview,
  AnimationPreviewStore,
  type BanterProjection,
  DEFAULT_ANIMATION_PREVIEW,
  type Projection,
  ProjectionStore,
} from "@code-musume/protocol";
import { Channel, invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export { invoke, isTauri };

type SnapshotStream = "projection" | "banter";

async function subscribeSnapshot<T>(
  stream: SnapshotStream,
  accept: (snapshot: T) => void,
): Promise<() => void> {
  const subscriptionId = crypto.randomUUID();
  let receiveTicket!: (ticket: number) => void;
  const offered = new Promise<number>((resolve) => {
    receiveTicket = resolve;
  });
  let stopped = false;
  const onUpdate = new Channel<{ snapshot: T } | { ticket: number }>((update) => {
    if (stopped) return;
    if ("ticket" in update) receiveTicket(update.ticket);
    else accept(update.snapshot);
  });
  const stop = () => {
    stopped = true;
    void invoke(`unsubscribe_${stream}`, { subscriptionId }).catch(() => {});
  };
  let timeout: ReturnType<typeof setTimeout> | undefined;
  const deadline = new Promise<never>((_resolve, reject) => {
    timeout = setTimeout(() => reject(new Error("桌宠订阅确认超时")), 4000);
  });
  const handshake = async () => {
    const [, ticket] = await Promise.all([
      invoke(`subscribe_${stream}`, { subscriptionId, onUpdate }),
      offered,
    ]);
    if (stopped) return;
    const snapshot = await invoke<T>(`confirm_${stream}`, { subscriptionId, ticket });
    if (!stopped) accept(snapshot);
  };
  try {
    await Promise.race([handshake(), deadline]);
    return stop;
  } catch (error) {
    stop();
    throw error;
  } finally {
    clearTimeout(timeout);
  }
}

function watchSnapshots<T>(stream: SnapshotStream, accept: (snapshot: T) => void) {
  // A WebView that lost the initial channel message can start a fresh handshake once.
  return subscribeSnapshot(stream, accept).catch(() => subscribeSnapshot(stream, accept));
}

export async function watchBanter(callback: (p: BanterProjection) => void): Promise<() => void> {
  if (!isTauri()) return () => {};
  let revision = -1;
  return watchSnapshots<BanterProjection>("banter", (p) => {
    if (p.revision > revision) {
      revision = p.revision;
      callback(p);
    }
  });
}
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
export async function watchProjection(
  callback: (p: Projection) => void,
  store = new ProjectionStore(),
): Promise<() => void> {
  if (!isTauri()) {
    callback(preview);
    return () => {};
  }
  return watchSnapshots<Projection>("projection", (p) => {
    if (store.accept(p)) callback(p);
  });
}
