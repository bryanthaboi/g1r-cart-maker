import { describe, expect, it } from "vitest";
import {
  canRedo,
  canUndo,
  createHistory,
  currentState,
  push,
  redo,
  reset,
  seal,
  undo,
} from "./history";

describe("history", () => {
  it("undoes and redoes in order", () => {
    let history = createHistory("a");
    history = push(history, "b", { label: "b" });
    history = push(history, "c", { label: "c" });
    expect(currentState(history)).toBe("c");
    history = undo(history);
    expect(currentState(history)).toBe("b");
    history = undo(history);
    expect(currentState(history)).toBe("a");
    expect(canUndo(history)).toBe(false);
    history = redo(history);
    expect(currentState(history)).toBe("b");
    expect(canRedo(history)).toBe(true);
  });

  it("coalesces a drag into a single entry", () => {
    let history = createHistory("a");
    history = push(history, "drag1", { label: "move", coalesceKey: "move:1" });
    history = push(history, "drag2", { label: "move", coalesceKey: "move:1" });
    history = push(history, "drag3", { label: "move", coalesceKey: "move:1" });
    expect(currentState(history)).toBe("drag3");
    expect(history.past).toHaveLength(1);
    history = undo(history);
    expect(currentState(history)).toBe("a");
  });

  it("starts a new entry once the run is sealed", () => {
    let history = createHistory("a");
    history = push(history, "b", { label: "move", coalesceKey: "move:1" });
    history = seal(history);
    history = push(history, "c", { label: "move", coalesceKey: "move:1" });
    expect(history.past).toHaveLength(2);
  });

  it("keys coalescing by target so two drags stay separate", () => {
    let history = createHistory("a");
    history = push(history, "b", { label: "move", coalesceKey: "move:1" });
    history = push(history, "c", { label: "move", coalesceKey: "move:2" });
    expect(history.past).toHaveLength(2);
  });

  it("drops the oldest entry past the limit", () => {
    let history = createHistory(0, 3);
    for (let step = 1; step <= 10; step += 1) {
      history = push(history, step, { label: `step ${step}` });
    }
    expect(history.past).toHaveLength(3);
    expect(history.past[0]?.state).toBe(7);
    expect(currentState(history)).toBe(10);
  });

  it("discards the redo branch after a new edit", () => {
    let history = createHistory("a");
    history = push(history, "b", { label: "b" });
    history = undo(history);
    history = push(history, "c", { label: "c" });
    expect(canRedo(history)).toBe(false);
    expect(currentState(history)).toBe("c");
  });

  it("undo and redo at the ends are no-ops", () => {
    const history = createHistory("a");
    expect(undo(history)).toBe(history);
    expect(redo(history)).toBe(history);
  });

  it("reset adopts an external state and clears both stacks", () => {
    let history = createHistory("a");
    history = push(history, "b", { label: "b" });
    history = reset(history, "outside");
    expect(currentState(history)).toBe("outside");
    expect(canUndo(history)).toBe(false);
    expect(canRedo(history)).toBe(false);
  });
});
