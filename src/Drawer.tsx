import { useState } from "react";
import { bubble, Task } from "./Office";

/** Danh sach cho duyet (☰): task Cho sip va Cho anh. */
export default function Drawer({ tasks, names, onApprove, onRework, onCancel, onOpen, onClose }: {
  tasks: Task[]; names: Record<string, string>;
  onApprove: (t: Task) => void; onRework: (t: Task, note: string) => void; onCancel: (t: Task) => void; onOpen: (t: Task) => void; onClose: () => void;
}) {
  const [note, setNote] = useState<{ id: string; text: string } | null>(null);
  const items = tasks.filter((t) => t.status === "review" || t.status === "waiting").sort((a, b) => b.created - a.created);

  return (
    <div className="drawer">
      <div className="history-head">
        Cho duyet ({items.length}) <button onClick={onClose}>✕</button>
      </div>
      <div className="history-list">
        {items.length === 0 && <div className="hint">Khong co viec nao cho duyet.</div>}
        {items.map((t) => (
          <div key={t.id} className="drawer-item">
            <div className="drawer-top">
              <b>{names[`${t.dept}/${t.agent}`] ?? t.agent}</b>
              <span className={`bubble ${t.status}`}>{bubble(t)}</span>
            </div>
            <div className="drawer-title">{t.title}</div>
            <div className="d">{new Date(t.created).toLocaleString()}</div>
            {t.summary && <div className="drawer-sum">{t.summary}</div>}
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
      </div>
    </div>
  );
}
