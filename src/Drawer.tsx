import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { bubble, Task } from "./Office";

/** Nhanh git cua task: chi co khi task chay trong worktree rieng (cwd nam trong .office/worktrees). */
const branch = (t: Task) => (/[\\/]\.office[\\/]worktrees[\\/]/.test(t.cwd) ? `office/${t.id}` : "");

/** ☰: tab "Cho duyet" (Cho sip + Cho anh) va tab "Lich su" (task da duyet/huy). */
export default function Drawer({ initialTab, tasks, names, onApprove, onRework, onCancel, onOpen, onClose }: {
  initialTab: "pending" | "history"; tasks: Task[]; names: Record<string, string>;
  onApprove: (t: Task) => void; onRework: (t: Task, note: string) => void; onCancel: (t: Task) => void; onOpen: (t: Task) => void; onClose: () => void;
}) {
  const [tab, setTab] = useState(initialTab);
  const [note, setNote] = useState<{ id: string; text: string } | null>(null);
  const items = tasks.filter((t) => t.status === "review" || t.status === "waiting").sort((a, b) => b.created - a.created);
  const done = tasks.filter((t) => t.status === "done").sort((a, b) => (b.finished || b.created) - (a.finished || a.created));

  // Xu ly het viec cuoi cung -> tu dong dong. Mo ra luc dang rong thi giu nguyen de sep xem lich su.
  const had = useRef(items.length);
  useEffect(() => {
    if (tab === "pending" && had.current > 0 && items.length === 0) onClose();
    had.current = items.length;
  }, [items.length, tab]);

  const reveal = (path: string) => invoke("open_in_editor", { path }).catch(() => {});
  const when = (ms: number) => new Date(ms).toLocaleString();

  return (
    <>
      <div className="drawer-backdrop" onClick={onClose} />
      <div className="drawer">
        <div className="history-head">
          <span className="drawer-tabs">
            <button className={tab === "pending" ? "on" : ""} onClick={() => setTab("pending")}>Cho duyet ({items.length})</button>
            <button className={tab === "history" ? "on" : ""} onClick={() => setTab("history")}>Lich su ({done.length})</button>
          </span>
          <button onClick={onClose}>✕</button>
        </div>
        <div className="history-list">
          {tab === "pending" && items.length === 0 && <div className="hint">Khong co viec nao cho duyet.</div>}
          {tab === "pending" && items.map((t) => (
            <div key={t.id} className="drawer-item">
              <div className="drawer-top">
                <b>{names[`${t.dept}/${t.agent}`] ?? t.agent}</b>
                <span className={`bubble ${t.status}`}>{bubble(t)}</span>
              </div>
              <div className="drawer-title">{t.title}</div>
              <div className="d">{when(t.created)} · {t.id}</div>
              {t.summary && <div className="drawer-sum">{t.summary}</div>}
              {t.status === "waiting" && t.ask && <div className="drawer-sum">{t.ask}</div>}
              {note?.id === t.id && (
                <textarea autoFocus placeholder="Nhan xet de lam lai..." value={note.text} onChange={(e) => setNote({ id: t.id, text: e.target.value })} />
              )}
              <div className="set-actions">
                {t.status === "waiting" && <button title="Bo task, giai phong cho" onClick={() => onCancel(t)}>Huy</button>}
                <button onClick={() => onOpen(t)}>Mo phien</button>
                {t.status === "review" && (note?.id === t.id ? (
                  <>
                    <button onClick={() => setNote(null)}>Huy</button>
                    <button className="primary" disabled={!note.text.trim()} onClick={() => (onRework(t, note.text.trim()), setNote(null))}>Gui lam lai</button>
                  </>
                ) : (
                  <>
                    <button onClick={() => setNote({ id: t.id, text: "" })}>Lam lai</button>
                    <button className="primary" onClick={() => onApprove(t)}>Duyet</button>
                  </>
                ))}
              </div>
            </div>
          ))}
          {tab === "history" && done.length === 0 && <div className="hint">Chua co task nao hoan thanh.</div>}
          {tab === "history" && done.map((t) => (
            <div key={t.id} className="drawer-item">
              <div className="drawer-top">
                <b>{names[`${t.dept}/${t.agent}`] ?? t.agent}</b>
                <span className={`bubble ${t.reason === "da huy" ? "waiting" : "done"}`}>{t.reason === "da huy" ? "Da huy" : "Da duyet"}</span>
              </div>
              <div className="drawer-title">{t.title}</div>
              <div className="d">{t.finished ? `xong ${when(t.finished)}` : `tao ${when(t.created)}`}</div>
              <div className="d">id: {t.id}{branch(t) && <> · nhanh: <code>{branch(t)}</code></>}</div>
              {t.summary && <div className="drawer-sum">{t.summary}</div>}
              <div className="set-actions">
                <button title={t.dir} onClick={() => reveal(t.dir)}>Mo ghi chu</button>
                {branch(t) && <button title={t.cwd} onClick={() => reveal(t.cwd)}>Mo worktree</button>}
              </div>
            </div>
          ))}
        </div>
      </div>
    </>
  );
}
