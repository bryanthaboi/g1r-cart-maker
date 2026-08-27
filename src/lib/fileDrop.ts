// Dropping a folder onto the window. Tauri intercepts the webview's own drag
// events, so the native channel is used there and HTML drag events elsewhere.

import { useEffect, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { IS_TAURI } from "./backend";
import { droppedPaths } from "./dialogs";

export interface FileDropState {
  hovering: boolean;
}

export function useFileDrop(onPaths: (paths: string[]) => void): FileDropState {
  const [hovering, setHovering] = useState(false);

  useEffect(() => {
    if (IS_TAURI) {
      let unlisten: (() => void) | null = null;
      let cancelled = false;
      void getCurrentWebview()
        .onDragDropEvent((event) => {
          const payload = event.payload;
          if (payload.type === "enter" || payload.type === "over") setHovering(true);
          else if (payload.type === "leave") setHovering(false);
          else if (payload.type === "drop") {
            setHovering(false);
            if (payload.paths.length > 0) onPaths(payload.paths.slice());
          }
        })
        .then((fn) => {
          if (cancelled) fn();
          else unlisten = fn;
        })
        .catch(() => {
          setHovering(false);
        });
      return () => {
        cancelled = true;
        if (unlisten) unlisten();
      };
    }

    const onOver = (event: DragEvent) => {
      event.preventDefault();
      setHovering(true);
    };
    const onLeave = (event: DragEvent) => {
      if (event.relatedTarget === null) setHovering(false);
    };
    const onDrop = (event: DragEvent) => {
      event.preventDefault();
      setHovering(false);
      const paths = droppedPaths(event);
      if (paths.length > 0) onPaths(paths);
    };
    window.addEventListener("dragover", onOver);
    window.addEventListener("dragleave", onLeave);
    window.addEventListener("drop", onDrop);
    return () => {
      window.removeEventListener("dragover", onOver);
      window.removeEventListener("dragleave", onLeave);
      window.removeEventListener("drop", onDrop);
    };
  }, [onPaths]);

  return { hovering };
}
