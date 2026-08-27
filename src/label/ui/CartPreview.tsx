// How the launcher will show the cart: shell colour, label, and the chosen finish.
// The finish here is an approximation of the engine's shader, not the shader itself.

import { useEffect, useMemo, useRef } from "react";
import type { Finish, LabelDoc } from "../../lib/types";
import { mixHex, normaliseHex, rgba } from "../core/colour";
import { contextOf, makeCanvas } from "../core/exportPng";
import { drawDoc, roundedRectPath, type ImageResolver } from "../core/render";

export interface CartPreviewProps {
  doc: LabelDoc;
  resolve: ImageResolver;
  shell: string;
  finish: Finish | null;
  redrawToken: number;
}

const WIDTH = 240;
const HEIGHT = 268;

interface Twinkle {
  x: number;
  y: number;
  phase: number;
  size: number;
}

function twinkles(count: number): Twinkle[] {
  const output: Twinkle[] = [];
  for (let index = 0; index < count; index += 1) {
    const seed = (index * 9301 + 49297) % 233280;
    const other = ((index + 7) * 4021 + 1) % 233280;
    output.push({
      x: (seed / 233280) * 0.9 + 0.05,
      y: (other / 233280) * 0.9 + 0.05,
      phase: ((index * 37) % 100) / 100,
      size: 1 + ((index * 13) % 5) * 0.4,
    });
  }
  return output;
}

export default function CartPreview(props: CartPreviewProps): JSX.Element {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const labelRef = useRef<HTMLCanvasElement | null>(null);
  const frameRef = useRef(0);
  const sparks = useMemo(() => twinkles(28), []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ratio = window.devicePixelRatio || 1;
    canvas.width = Math.round(WIDTH * ratio);
    canvas.height = Math.round(HEIGHT * ratio);
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let label = labelRef.current;
    if (!label || label.width !== props.doc.width || label.height !== props.doc.height) {
      label = makeCanvas(props.doc.width, props.doc.height);
      labelRef.current = label;
    }
    try {
      const labelCtx = contextOf(label);
      labelCtx.setTransform(1, 0, 0, 1, 0, 0);
      labelCtx.clearRect(0, 0, label.width, label.height);
      drawDoc(labelCtx, props.doc, props.resolve);
    } catch {
      labelRef.current = null;
    }

    const reduced =
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    const shell = normaliseHex(props.shell, "#8a8f98");
    const labelBox = { x: 24, y: 30, width: WIDTH - 48, height: (WIDTH - 48) * (441 / 500) };

    const render = (time: number): void => {
      ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
      ctx.clearRect(0, 0, WIDTH, HEIGHT);

      const body = { x: 8, y: 8, width: WIDTH - 16, height: HEIGHT - 16 };
      const shade = ctx.createLinearGradient(body.x, body.y, body.x + body.width, body.y + body.height);
      shade.addColorStop(0, mixHex(shell, "#ffffff", 0.22));
      shade.addColorStop(0.55, shell);
      shade.addColorStop(1, mixHex(shell, "#000000", 0.28));
      roundedRectPath(ctx, body, 16);
      ctx.fillStyle = shade;
      ctx.fill();
      ctx.strokeStyle = rgba(mixHex(shell, "#000000", 0.5), 0.7);
      ctx.lineWidth = 1.5;
      ctx.stroke();

      ctx.save();
      roundedRectPath(ctx, body, 16);
      ctx.clip();
      ctx.fillStyle = rgba(mixHex(shell, "#000000", 0.35), 0.5);
      for (let index = 0; index < 5; index += 1) {
        ctx.fillRect(body.x + 22 + index * 14, body.y + body.height - 52, 6, 40);
      }
      ctx.fillStyle = rgba(mixHex(shell, "#000000", 0.4), 0.35);
      ctx.fillRect(body.x, body.y + body.height - 74, body.width, 3);
      ctx.restore();

      ctx.save();
      roundedRectPath(ctx, labelBox, 8);
      ctx.clip();
      ctx.fillStyle = mixHex(shell, "#ffffff", 0.85);
      ctx.fillRect(labelBox.x, labelBox.y, labelBox.width, labelBox.height);
      const source = labelRef.current;
      if (source) {
        ctx.drawImage(source, labelBox.x, labelBox.y, labelBox.width, labelBox.height);
      }

      const seconds = reduced ? 0.35 : time / 1000;
      const finish = props.finish;
      if (finish === "holo" || finish === "sparkle+holo") {
        const sweep = ((seconds * 0.16) % 1) * 2 - 0.5;
        const gradient = ctx.createLinearGradient(
          labelBox.x + labelBox.width * (sweep - 0.4),
          labelBox.y,
          labelBox.x + labelBox.width * (sweep + 0.6),
          labelBox.y + labelBox.height,
        );
        gradient.addColorStop(0, "rgba(255, 0, 128, 0)");
        gradient.addColorStop(0.25, "rgba(255, 64, 96, 0.30)");
        gradient.addColorStop(0.45, "rgba(255, 220, 64, 0.30)");
        gradient.addColorStop(0.6, "rgba(64, 255, 180, 0.30)");
        gradient.addColorStop(0.78, "rgba(96, 128, 255, 0.30)");
        gradient.addColorStop(1, "rgba(160, 0, 255, 0)");
        ctx.globalCompositeOperation = "screen";
        ctx.fillStyle = gradient;
        ctx.fillRect(labelBox.x, labelBox.y, labelBox.width, labelBox.height);
        ctx.globalCompositeOperation = "source-over";
      }
      if (finish === "sparkle" || finish === "sparkle+holo") {
        const sweep = ((seconds * 0.3) % 1.6) - 0.3;
        const shine = ctx.createLinearGradient(
          labelBox.x + labelBox.width * (sweep - 0.18),
          labelBox.y,
          labelBox.x + labelBox.width * (sweep + 0.18),
          labelBox.y + labelBox.height,
        );
        shine.addColorStop(0, "rgba(255, 255, 255, 0)");
        shine.addColorStop(0.5, "rgba(255, 255, 255, 0.28)");
        shine.addColorStop(1, "rgba(255, 255, 255, 0)");
        ctx.fillStyle = shine;
        ctx.fillRect(labelBox.x, labelBox.y, labelBox.width, labelBox.height);
        for (const spark of sparks) {
          const pulse = 0.5 + 0.5 * Math.sin(seconds * 3 + spark.phase * Math.PI * 2);
          if (pulse < 0.35) continue;
          const cx = labelBox.x + spark.x * labelBox.width;
          const cy = labelBox.y + spark.y * labelBox.height;
          const size = spark.size * (0.6 + pulse);
          ctx.fillStyle = `rgba(255, 255, 255, ${(pulse * 0.85).toFixed(3)})`;
          ctx.beginPath();
          ctx.moveTo(cx, cy - size * 2);
          ctx.lineTo(cx + size * 0.6, cy);
          ctx.lineTo(cx, cy + size * 2);
          ctx.lineTo(cx - size * 0.6, cy);
          ctx.closePath();
          ctx.fill();
        }
      }
      ctx.restore();

      roundedRectPath(ctx, labelBox, 8);
      ctx.strokeStyle = rgba(mixHex(shell, "#000000", 0.55), 0.55);
      ctx.lineWidth = 1;
      ctx.stroke();

      if (!reduced) frameRef.current = window.requestAnimationFrame(render);
    };

    frameRef.current = window.requestAnimationFrame(render);
    return () => window.cancelAnimationFrame(frameRef.current);
  }, [props.doc, props.finish, props.redrawToken, props.resolve, props.shell, sparks]);

  return (
    <div className="ld-preview">
      <canvas ref={canvasRef} style={{ aspectRatio: `${WIDTH} / ${HEIGHT}` }} />
      <p className="ld-note">
        Cartridge preview: shell {normaliseHex(props.shell, "#8a8f98")}, finish {props.finish ?? "none"}.
        The finish is an approximation of the launcher&apos;s shader.
      </p>
    </div>
  );
}
