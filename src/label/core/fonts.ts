// The font menu. Only families every desktop already has; the designer embeds no webfont.

export interface FontChoice {
  id: string;
  name: string;
  stack: string;
}

export const FONT_CHOICES: readonly FontChoice[] = [
  { id: "system", name: "System sans", stack: "system-ui, -apple-system, Segoe UI, Roboto, sans-serif" },
  { id: "grotesk", name: "Helvetica / Arial", stack: "Helvetica Neue, Helvetica, Arial, sans-serif" },
  { id: "geometric", name: "Futura / Century Gothic", stack: "Futura, Century Gothic, Trebuchet MS, sans-serif" },
  { id: "condensed", name: "Condensed", stack: "Arial Narrow, Roboto Condensed, Helvetica Neue, sans-serif" },
  { id: "impact", name: "Impact", stack: "Impact, Haettenschweiler, Arial Black, sans-serif" },
  { id: "serif", name: "Serif", stack: "Georgia, Times New Roman, serif" },
  { id: "slab", name: "Slab serif", stack: "Rockwell, Courier Bold, Georgia, serif" },
  { id: "mono", name: "Monospace", stack: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace" },
];

export const WEIGHTS: readonly string[] = ["300", "400", "500", "600", "700", "800", "900"];

export function fontChoiceFor(stack: string): FontChoice | null {
  return FONT_CHOICES.find((choice) => choice.stack === stack) ?? null;
}

export function defaultFontStack(): string {
  return FONT_CHOICES[0]?.stack ?? "sans-serif";
}
