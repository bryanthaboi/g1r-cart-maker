// Colour helpers. Every stored colour is #rrggbb; nothing here reads a design token.

const HEX = /^#[0-9a-fA-F]{6}$/;
const SHORT_HEX = /^#[0-9a-fA-F]{3}$/;

export function isHex(value: string): boolean {
  return HEX.test(value);
}

/** Accept #rgb, #rrggbb and a bare hex body; anything else keeps the fallback. */
export function normaliseHex(value: string, fallback = "#000000"): string {
  const text = value.trim();
  const candidate = text.startsWith("#") ? text : `#${text}`;
  if (HEX.test(candidate)) return candidate.toLowerCase();
  if (SHORT_HEX.test(candidate)) {
    const body = candidate.slice(1);
    const parts = [...body].map((digit) => `${digit}${digit}`).join("");
    return `#${parts}`.toLowerCase();
  }
  return fallback;
}

export function toRgb(value: string): { r: number; g: number; b: number } {
  const hex = normaliseHex(value);
  return {
    r: Number.parseInt(hex.slice(1, 3), 16),
    g: Number.parseInt(hex.slice(3, 5), 16),
    b: Number.parseInt(hex.slice(5, 7), 16),
  };
}

function channel(value: number): string {
  return Math.max(0, Math.min(255, Math.round(value))).toString(16).padStart(2, "0");
}

export function fromRgb(r: number, g: number, b: number): string {
  return `#${channel(r)}${channel(g)}${channel(b)}`;
}

export function mixHex(a: string, b: string, amount: number): string {
  const from = toRgb(a);
  const to = toRgb(b);
  const t = Math.max(0, Math.min(1, amount));
  return fromRgb(
    from.r + (to.r - from.r) * t,
    from.g + (to.g - from.g) * t,
    from.b + (to.b - from.b) * t,
  );
}

export function luminance(value: string): number {
  const { r, g, b } = toRgb(value);
  const linear = [r, g, b].map((raw) => {
    const scaled = raw / 255;
    return scaled <= 0.03928 ? scaled / 12.92 : ((scaled + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * (linear[0] ?? 0) + 0.7152 * (linear[1] ?? 0) + 0.0722 * (linear[2] ?? 0);
}

/** Readable ink for a background, so generated text is never invisible. */
export function inkFor(background: string): string {
  return luminance(background) > 0.45 ? "#151515" : "#ffffff";
}

export function rgba(value: string, alpha: number): string {
  const { r, g, b } = toRgb(value);
  return `rgba(${r}, ${g}, ${b}, ${Math.max(0, Math.min(1, alpha))})`;
}
