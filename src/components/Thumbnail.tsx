import { useEffect, useState } from "react";
import { cachedThumbnail, loadThumbnail } from "../lib/thumbnails";

type Status = "idle" | "loading" | "ready" | "failed";

/** A thumbnail that fails is a blank tile, never a broken list row. */
export function Thumbnail({ url, alt }: { url: string | null; alt: string }): JSX.Element {
  const cached = url ? cachedThumbnail(url) : null;
  const [status, setStatus] = useState<Status>(url ? (cached ? "ready" : "loading") : "idle");
  const [dataUrl, setDataUrl] = useState<string | null>(cached);

  useEffect(() => {
    let cancelled = false;
    if (!url) {
      setStatus("idle");
      setDataUrl(null);
      return () => {
        cancelled = true;
      };
    }
    const hit = cachedThumbnail(url);
    if (hit) {
      setDataUrl(hit);
      setStatus("ready");
      return () => {
        cancelled = true;
      };
    }
    setStatus("loading");
    loadThumbnail(url)
      .then((value) => {
        if (cancelled) return;
        setDataUrl(value);
        setStatus("ready");
      })
      .catch(() => {
        if (cancelled) return;
        setDataUrl(null);
        setStatus("failed");
      });
    return () => {
      cancelled = true;
    };
  }, [url]);

  if (status === "ready" && dataUrl) {
    return <img className="thumb" src={dataUrl} alt={alt} onError={() => setStatus("failed")} />;
  }
  return (
    <div className={`thumb thumb-blank${status === "loading" ? " thumb-loading" : ""}`} aria-hidden="true">
      <span>{alt.slice(0, 2).toUpperCase()}</span>
    </div>
  );
}
