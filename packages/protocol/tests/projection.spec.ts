import { expect, it } from "vitest";
import { type Projection, ProjectionStore } from "../src/index";

function p(instanceId: string, revision: number): Projection {
  return {
    v: 2,
    instanceId,
    revision,
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
}
it("late initial snapshots and repeated updates cannot replace a newer frame", () => {
  const s = new ProjectionStore();
  expect(s.accept(p("a", 3))).toBe(true);
  expect(s.accept(p("a", 1))).toBe(false);
  expect(s.accept(p("a", 3))).toBe(false);
  expect(s.current?.revision).toBe(3);
});
it("reconnect accepts a new instance and rejects late data from the retired instance", () => {
  const s = new ProjectionStore();
  s.accept(p("a", 100));
  expect(s.accept(p("b", 0))).toBe(true);
  expect(s.accept(p("a", 101))).toBe(false);
  expect(s.current?.instanceId).toBe("b");
});
it("mismatched protocol and unsafe revisions are rejected", () => {
  const s = new ProjectionStore();
  expect(s.accept({ ...p("a", 1), v: 1 })).toBe(false);
  expect(s.accept(p("a", Number.NaN))).toBe(false);
  expect(s.accept(p("a", -1))).toBe(false);
});
