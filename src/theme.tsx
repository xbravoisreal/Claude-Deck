import { createContext, useContext, useEffect, useState } from "react";

/** Theme tu Rust (theme.rs): mau, font, kich thuoc sprite, slot -> data URL. */
export type Theme = {
  name: string;
  colors: Record<string, string>;
  font: string;
  sprite: { frame: number; scale: number; characterY?: number; deskY?: number; signSlice?: number; shelfSlice?: number };
  slots: Record<string, string>;
  errors: string[];
};

export const ThemeCtx = createContext<Theme | null>(null);
export const useTheme = () => useContext(ThemeCtx);

/** Kich thuoc mot o (px) = khung sprite x he so phong. */
export const cellOf = (t: Theme) => t.sprite.frame * t.sprite.scale;

/** Mau theme -> CSS variable `--o-<ten>` (component chi dung bien, khong ma mau). */
export const themeVars = (t: Theme) =>
  ({
    ...Object.fromEntries(Object.entries(t.colors).map(([k, v]) => [`--o-${k}`, v])),
    "--o-font": t.font,
    "--o-cell": `${cellOf(t)}px`,
  }) as React.CSSProperties;

/** Id cac nhan vat co trong theme (slot character.<id>.typing). */
export const characters = (t: Theme) =>
  Object.keys(t.slots).flatMap((k) => k.match(/^character\.(.+)\.typing$/)?.slice(1) ?? []).sort();

const frameCache = new Map<string, number>();

/** So khung cua anh = rong anh / frame (anh nhieu khung xep ngang). */
function useFrames(url: string | undefined, frame: number) {
  const [n, setN] = useState(() => (url && frameCache.get(url)) || 1);
  useEffect(() => {
    if (!url) return;
    if (frameCache.has(url)) return setN(frameCache.get(url)!);
    const img = new Image();
    img.onload = () => {
      const k = Math.max(1, Math.round(img.naturalWidth / frame));
      frameCache.set(url, k);
      setN(k);
    };
    img.src = url;
  }, [url, frame]);
  return n;
}

/** Ve mot slot. Anh nhieu khung tu chay animation; slot thieu thi khong ve gi. */
export function Sprite({ slot, className, style }: { slot: string; className?: string; style?: React.CSSProperties }) {
  const t = useTheme()!;
  const url = t.slots[slot];
  const n = useFrames(url, t.sprite.frame);
  if (!url) return null;
  return (
    <div
      className={`sprite${className ? ` ${className}` : ""}`}
      style={{ backgroundImage: `url(${url})`, ["--n" as string]: n, animationName: n > 1 ? "sprite-run" : "none", ...style }}
    />
  );
}
