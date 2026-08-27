import { beforeEach, describe, expect, it, vi } from "vitest";

const thumbnail = vi.fn();
vi.mock("./backend", () => ({ api: { feeds: { thumbnail: (url: string) => thumbnail(url) } } }));

import { cachedThumbnail, loadThumbnail, resetThumbnails } from "./thumbnails";

async function flush(): Promise<void> {
  for (let i = 0; i < 10; i += 1) await Promise.resolve();
}

function deferred(): { promise: Promise<string>; resolve: (value: string) => void } {
  let resolve!: (value: string) => void;
  const promise = new Promise<string>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

describe("thumbnail loading", () => {
  beforeEach(() => {
    resetThumbnails();
    thumbnail.mockReset();
  });

  it("runs at most six requests at once", async () => {
    const gates = Array.from({ length: 10 }, () => deferred());
    let issued = 0;
    thumbnail.mockImplementation(() => gates[issued++]!.promise);

    const all = Array.from({ length: 10 }, (_, i) => loadThumbnail(`u${i}`).catch(() => ""));
    await flush();
    expect(issued).toBe(6);

    gates[0]!.resolve("a");
    await flush();
    expect(issued).toBe(7);

    gates.slice(1).forEach((gate, i) => gate.resolve(`v${i}`));
    await Promise.all(all);
  });

  it("fetches one url once and serves the rest from cache", async () => {
    thumbnail.mockResolvedValue("data:image/png;base64,AA");
    const first = await loadThumbnail("same");
    const second = await loadThumbnail("same");
    expect(first).toBe(second);
    expect(thumbnail).toHaveBeenCalledTimes(1);
    expect(cachedThumbnail("same")).toBe("data:image/png;base64,AA");
  });

  it("shares one in-flight request between simultaneous callers", async () => {
    const gate = deferred();
    thumbnail.mockReturnValue(gate.promise);
    const a = loadThumbnail("shared");
    const b = loadThumbnail("shared");
    gate.resolve("done");
    expect(await a).toBe("done");
    expect(await b).toBe("done");
    expect(thumbnail).toHaveBeenCalledTimes(1);
  });

  it("remembers a failure instead of retrying it on every render", async () => {
    thumbnail.mockRejectedValue(new Error("404"));
    await expect(loadThumbnail("gone")).rejects.toThrow();
    await expect(loadThumbnail("gone")).rejects.toThrow();
    expect(thumbnail).toHaveBeenCalledTimes(1);
    expect(cachedThumbnail("gone")).toBeNull();
  });

  it("releases its slot after a failure so the queue keeps moving", async () => {
    thumbnail.mockRejectedValue(new Error("404"));
    const all = Array.from({ length: 12 }, (_, i) => loadThumbnail(`f${i}`).catch(() => ""));
    await Promise.all(all);
    expect(thumbnail).toHaveBeenCalledTimes(12);
  });
});
