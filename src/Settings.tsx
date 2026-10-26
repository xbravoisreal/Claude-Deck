import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

/** notes "" = khong dung vault Obsidian. maxLive: so phien task song toi da. warnK: nguong canh bao ngu canh (nghin token).
 *  theme: ten thu muc trong <root>/themes. tgChat: chat id Telegram ("" = tat bot). Token bot KHONG nam o day (Credential Manager). */
export type OfficePrefs = { root: string; notes: string; maxLive: number; warnK: number; theme: string; tgChat: string };
export const OFFICE_DEFAULTS: OfficePrefs = { root: "", notes: "", maxLive: 3, warnK: 150, theme: "default", tgChat: "" };

type Dir = "root" | "notes";
type Num = "maxLive" | "warnK";

export default function Settings({ value, onSave, onClose }: { value: OfficePrefs; onSave: (v: OfficePrefs) => void; onClose: () => void }) {
  const [v, setV] = useState(value);
  const [themes, setThemes] = useState<string[]>([value.theme]);
  // Token chi di mot chieu xuong Rust; o nhap de trong = giu token da luu.
  const [token, setToken] = useState("");
  const [hasToken, setHasToken] = useState(false);
  const [tg, setTg] = useState("");
  const [chats, setChats] = useState<[number, string][] | null>(null);
  useEffect(() => {
    if (v.root) invoke<string[]>("theme_list", { root: v.root }).then(setThemes, () => {});
  }, [v.root]);
  // Ghi thang vao registry khi bam (khong cho nut Luu).
  const [autostart, setAutostart] = useState(false);
  useEffect(() => {
    invoke<boolean>("autostart_get").then(setAutostart, () => {});
    invoke<boolean>("tg_has_token").then(setHasToken, () => {});
    invoke<string>("tg_status").then(setTg, () => {});
  }, []);

  const saveToken = async () => {
    if (!token.trim()) return;
    await invoke("tg_set_token", { token });
    setToken("");
    setHasToken(true);
  };
  const run = (f: () => Promise<unknown>, ok: string) => f().then(() => setTg(ok), (e) => setTg(`loi: ${e}`));

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

        <label className="set-field set-check">
          <input type="checkbox" checked={autostart} onChange={(e) => invoke("autostart_set", { on: e.target.checked }).then(() => setAutostart(!autostart), (err) => setTg(`loi: ${err}`))} />
          <span>Chay cung Windows (vao thang khay; routine va bot chi chay khi app mo)</span>
        </label>

        <div className="set-sub">Telegram</div>
        <label className="set-field">
          <span>Bot token (luu trong Windows Credential Manager)</span>
          <div>
            <input type="password" autoComplete="off" value={token} placeholder={hasToken ? "da luu - nhap de thay" : "123456:ABC... tu @BotFather"} onChange={(e) => setToken(e.target.value)} />
            {hasToken && <button title="Xoa token khoi Credential Manager" onClick={() => run(() => invoke("tg_set_token", { token: "" }).then(() => setHasToken(false)), "da xoa token")}>Xoa</button>}
          </div>
        </label>
        <label className="set-field">
          <span>Chat id (nhan cho bot 1 cau roi bam Tim)</span>
          <div>
            <input value={v.tgChat} placeholder="vd 123456789" onChange={(e) => setV({ ...v, tgChat: e.target.value.trim() })} />
            <button onClick={() => run(async () => (await saveToken(), setChats(await invoke<[number, string][]>("tg_detect_chat"))), "")}>Tim</button>
          </div>
        </label>
        {chats && (
          <div className="tg-chats">
            {chats.length === 0 && <span className="hint">Chua thay tin nao. Nhan cho bot 1 cau roi bam Tim lai.</span>}
            {chats.map(([id, name]) => (
              <button key={id} onClick={() => (setV({ ...v, tgChat: String(id) }), setChats(null))}>{name} · {id}</button>
            ))}
          </div>
        )}
        <div className="set-actions">
          {tg && <span className="tg-status">{tg}</span>}
          <button disabled={!v.tgChat || (!hasToken && !token.trim())} onClick={() => run(async () => (await saveToken(), await invoke("tg_test", { chatId: v.tgChat })), "da gui tin thu")}>Gui thu</button>
        </div>

        <div className="set-actions">
          <button onClick={onClose}>Huy</button>
          <button className="primary" onClick={() => saveToken().then(() => (onSave({ ...v, root: v.root.trim(), notes: v.notes.trim() }), onClose()), (e) => setTg(`loi: ${e}`))}>Luu</button>
        </div>
      </div>
    </div>
  );
}
