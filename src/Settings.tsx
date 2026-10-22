import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

/** notes "" = khong dung vault Obsidian. maxLive: so phien task song toi da. warnK: nguong canh bao ngu canh (nghin token). theme: ten thu muc trong <root>/themes. */
export type OfficePrefs = { root: string; notes: string; maxLive: number; warnK: number; theme: string };
export const OFFICE_DEFAULTS: OfficePrefs = { root: "", notes: "", maxLive: 3, warnK: 150, theme: "default" };

type Dir = "root" | "notes";
type Num = "maxLive" | "warnK";

export default function Settings({ value, onSave, onClose }: { value: OfficePrefs; onSave: (v: OfficePrefs) => void; onClose: () => void }) {
  const [v, setV] = useState(value);
  const [themes, setThemes] = useState<string[]>([value.theme]);
  useEffect(() => {
    if (v.root) invoke<string[]>("theme_list", { root: v.root }).then(setThemes, () => {});
  }, [v.root]);
  const pick = async (key: Dir) => {
    const dir = await open({ directory: true, defaultPath: v[key] || undefined });
    if (typeof dir === "string") setV({ ...v, [key]: dir });
  };
  const field = (key: Dir, label: string, hint: string) => (
    <label className="set-field">
      <span>{label}</span>
      <div>
        <input value={v[key]} placeholder={hint} onChange={(e) => setV({ ...v, [key]: e.target.value })} />
        <button onClick={() => pick(key)}>Chon...</button>
      </div>
    </label>
  );
  const num = (key: Num, label: string) => (
    <label className="set-field">
      <span>{label}</span>
      <div>
        <input type="number" min={1} value={v[key]} onChange={(e) => setV({ ...v, [key]: Math.max(1, Number(e.target.value) || 1) })} />
      </div>
    </label>
  );

  return (
    <div className="history settings">
      <div className="history-head">
        Settings <button onClick={onClose}>✕</button>
      </div>
      <div className="history-list">
        {field("root", "Thu muc office", "")}
        {field("notes", "Vault Obsidian (notesRoot)", "Bo trong neu khong dung")}
        {num("maxLive", "Phien task song toi da")}
        {num("warnK", "Canh bao ngu canh (nghin token)")}
        <label className="set-field">
          <span>Theme Office</span>
          <div>
            <select value={v.theme} onChange={(e) => setV({ ...v, theme: e.target.value })}>
              {themes.map((t) => <option key={t}>{t}</option>)}
            </select>
            <button title="Copy default thanh thu muc moi de tao theme (xem README.md)" onClick={() => invoke("open_in_editor", { path: `${v.root}\\themes` }).catch(() => {})}>Mo thu muc theme</button>
          </div>
        </label>
        <div className="set-actions">
          <button onClick={onClose}>Huy</button>
          <button className="primary" onClick={() => (onSave({ ...v, root: v.root.trim(), notes: v.notes.trim() }), onClose())}>Luu</button>
        </div>
      </div>
    </div>
  );
}
