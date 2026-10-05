// The shipped label templates, used when the backend cannot serve them.

import type { Base, LabelTemplate } from "../lib/types";
import blueArt from "../../assets/labels/blue.png";
import crystalArt from "../../assets/labels/crystal.png";
import emeraldArt from "../../assets/labels/emerald.png";
import fireredArt from "../../assets/labels/firered.png";
import goldArt from "../../assets/labels/gold.png";
import leafgreenArt from "../../assets/labels/leafgreen.png";
import redArt from "../../assets/labels/red.png";
import rubyArt from "../../assets/labels/ruby.png";
import sapphireArt from "../../assets/labels/sapphire.png";
import silverArt from "../../assets/labels/silver.png";
import yellowArt from "../../assets/labels/yellow.png";
import { fetchAsDataUrl } from "./core/images";
import { canvasFor } from "./core/doc";

const ART: [Base, string, string][] = [
  ["red", "Red", redArt],
  ["blue", "Blue", blueArt],
  ["yellow", "Yellow", yellowArt],
  ["gold", "Gold", goldArt],
  ["silver", "Silver", silverArt],
  ["crystal", "Crystal", crystalArt],
  ["firered", "FireRed", fireredArt],
  ["leafgreen", "LeafGreen", leafgreenArt],
  ["ruby", "Ruby", rubyArt],
  ["sapphire", "Sapphire", sapphireArt],
  ["emerald", "Emerald", emeraldArt],
];

export async function bundledTemplates(): Promise<LabelTemplate[]> {
  const loaded = await Promise.all(
    ART.map(async ([base, name, url]) => {
      const dataUrl = await fetchAsDataUrl(url);
      return {
        id: base,
        name,
        base,
        ...canvasFor(base),
        dataUrl,
      } satisfies LabelTemplate;
    }),
  );
  return loaded;
}
