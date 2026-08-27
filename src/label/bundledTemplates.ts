// The six shipped label templates, used when the backend cannot serve them.

import type { Base, LabelTemplate } from "../lib/types";
import blueArt from "../../assets/labels/blue.png";
import crystalArt from "../../assets/labels/crystal.png";
import goldArt from "../../assets/labels/gold.png";
import redArt from "../../assets/labels/red.png";
import silverArt from "../../assets/labels/silver.png";
import yellowArt from "../../assets/labels/yellow.png";
import { fetchAsDataUrl } from "./core/images";
import { CANVAS_HEIGHT, CANVAS_WIDTH } from "./core/doc";

const ART: [Base, string, string][] = [
  ["red", "Red", redArt],
  ["blue", "Blue", blueArt],
  ["yellow", "Yellow", yellowArt],
  ["gold", "Gold", goldArt],
  ["silver", "Silver", silverArt],
  ["crystal", "Crystal", crystalArt],
];

export async function bundledTemplates(): Promise<LabelTemplate[]> {
  const loaded = await Promise.all(
    ART.map(async ([base, name, url]) => {
      const dataUrl = await fetchAsDataUrl(url);
      return {
        id: base,
        name,
        base,
        width: CANVAS_WIDTH,
        height: CANVAS_HEIGHT,
        dataUrl,
      } satisfies LabelTemplate;
    }),
  );
  return loaded;
}
