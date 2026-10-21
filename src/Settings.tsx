import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

/** notes "" = khong dung vault Obsidian. */
export type OfficePrefs = { root: string; notes: string };

export default function Settings({ value, onSave, onClose }: { value: OfficePrefs; onSave: (v: OfficePrefs) => void; onClose: () => void }) {
  const [v, setV] = useState(value);
  const pick = async (key: keyof OfficePrefs) => {
    const dir = await open({ directory: true, defaultPath: v[key] || undefined });
    if (typeof dir === "string") setV({ ...v, [key]: dir });
  };
  const field = (key: keyof OfficePrefs, label: string, hint: string) => (
    <label className="set-field">
      <span>{label}</span>
      <div>
        <input value={v[key]} placeholder={hint} onChange={(e) => setV({ ...v, [key]: e.target.value })} />
        <button onClick={() => pick(key)}>Chon...</button>
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
        <div className="set-actions">
          <button onClick={onClose}>Huy</button>
          <button className="primary" onClick={() => (onSave({ root: v.root.trim(), notes: v.notes.trim() }), onClose())}>Luu</button>
        </div>
      </div>
    </div>
  );
}
