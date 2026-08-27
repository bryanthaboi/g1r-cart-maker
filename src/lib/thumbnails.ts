import { api } from "./backend";

/// A feed of 150 mods asks for every thumbnail at once, so requests queue a few
/// at a time and each URL is fetched once per session.
const LIMIT = 6;

const ready = new Map<string, string>();
const failed = new Set<string>();
const inFlight = new Map<string, Promise<string>>();
const waiting: (() => void)[] = [];
let active = 0;

function release(): void {
  active -= 1;
  const next = waiting.shift();
  if (next) next();
}

function acquire(): Promise<void> {
  if (active < LIMIT) {
    active += 1;
    return Promise.resolve();
  }
  return new Promise<void>((resolve) => {
    waiting.push(() => {
      active += 1;
      resolve();
    });
  });
}

export function cachedThumbnail(url: string): string | null {
  return ready.get(url) ?? null;
}

export function loadThumbnail(url: string): Promise<string> {
  const done = ready.get(url);
  if (done !== undefined) return Promise.resolve(done);
  if (failed.has(url)) return Promise.reject(new Error("thumbnail unavailable"));
  const running = inFlight.get(url);
  if (running) return running;

  const task = acquire()
    .then(() => api.feeds.thumbnail(url))
    .then((value) => {
      ready.set(url, value);
      return value;
    })
    .catch((problem: unknown) => {
      failed.add(url);
      throw problem;
    })
    .finally(() => {
      release();
      inFlight.delete(url);
    });

  inFlight.set(url, task);
  return task;
}

/** Test seam: forget every cached and failed URL. */
export function resetThumbnails(): void {
  ready.clear();
  failed.clear();
  inFlight.clear();
  waiting.length = 0;
  active = 0;
}
