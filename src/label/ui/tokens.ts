// The overlay paints with the app's design tokens; nothing here holds a literal colour.

export interface StageColours {
  surface: string;
  raised: string;
  border: string;
  focus: string;
  accent: string;
  faint: string;
  text: string;
}

const NAMES: Record<keyof StageColours, string> = {
  surface: "--bg-sunken",
  raised: "--bg-raised",
  border: "--border",
  focus: "--border-focus",
  accent: "--accent",
  faint: "--text-faint",
  text: "--text",
};

export function readColours(element: Element): StageColours {
  const style = window.getComputedStyle(element);
  const fallback = style.color;
  const read = (token: string): string => {
    const value = style.getPropertyValue(token).trim();
    return value.length > 0 ? value : fallback;
  };
  return {
    surface: read(NAMES.surface),
    raised: read(NAMES.raised),
    border: read(NAMES.border),
    focus: read(NAMES.focus),
    accent: read(NAMES.accent),
    faint: read(NAMES.faint),
    text: read(NAMES.text),
  };
}
