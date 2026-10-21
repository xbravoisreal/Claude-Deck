import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

/** notes "" = khong dung vault Obsidian. maxLive: so phien task song toi da. warnK: nguong canh bao ngu canh (nghin token). */
export type OfficePrefs = { root: string; notes: string; maxLive: number; warnK: number };
export const OFFICE_DEFAULTS: OfficePrefs = { root: "", notes: "", maxLive: 3, warnK: 150 };

type Dir = "root" | "notes";
type Num = "maxLive" | "warnK";

export default function Settings({ value, onSave, onClose }: { value: OfficePrefs; onSave: (v: OfficePrefs) => void; onClose: () => void }) {
  const [v, setV] = useState(value);
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
        <div className="set-actions">
          <button onClick={onClose}>Huy</button>
          <button className="primary" onClick={() => (onSave({ ...v, root: v.root.trim(), notes: v.notes.trim() }), onClose())}>Luu</button>
        </div>
      </div>
    </div>
  );
}
