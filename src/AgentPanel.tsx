import { useState } from "react";
import { bubble, type AgentDef, type Dept, type Task } from "./Office";

/** Panel chi tiet Agent (bam vao ban): thong tin, lich su task, Giao viec / Mo phien / Sua. */
const STATUS: Record<string, string> = { active: "Chính thức", probation: "Thử việc", retired: "Đã nghỉ" };

export default function AgentPanel({ dept, agent, tasks, onAssign, onLaunch, onEdit, onStatus, onClose }: {
  dept: Dept; agent: AgentDef; tasks: Task[];
  onAssign: (title: string, body: string) => Promise<unknown>; onLaunch: () => void; onEdit: () => void;
  onStatus: (status: string) => Promise<unknown>; onClose: () => void;
}) {
  const [retire, setRetire] = useState(false);
  const [error, setError] = useState("");
  const status = dept.status?.[agent.id] ?? "active";
  const change = (s: string) => onStatus(s).catch((e) => setError(String(e)));
  const [form, setForm] = useState<{ title: string; body: string } | null>(null);
  const [sending, setSending] = useState(false);
  const mine = tasks.filter((t) => t.dept === dept.id && t.agent === agent.id).sort((a, b) => b.created - a.created);
  const budget = dept.budget?.[agent.id] ?? dept.defaults?.maxTokens ?? 4_000_000;

  const submit = async () => {
    if (!form?.title.trim() || sending) return;
    setSending(true); // bam 2 lan khong tao 2 task
    try {
      await onAssign(form.title.trim(), form.body);
      setForm(null);
    } finally {
      setSending(false);
    }
  };

  return (
    <div className="history agent-panel">
      <div className="history-head">
        {dept.icon} {agent.name} <button onClick={onClose}>✕</button>
      </div>
      <div className="history-list">
        <div className="ap-desc">{agent.description}</div>
        <dl className="ap-meta">
          <dt>Model</dt><dd>{agent.model ?? "default"}</dd>
          <dt>Tools</dt><dd>{agent.tools.join(", ") || "ke thua"}</dd>
          <dt>Ngan sach</dt><dd>{(budget / 1e6).toLocaleString()}M token / task</dd>
          <dt>Trang thai</dt><dd>{STATUS[status] ?? status}</dd>
        </dl>
        {error && <div className="office-error" onClick={() => setError("")}>{error}</div>}
        <div className="set-actions">
          {status === "probation" && <button onClick={() => change("active")}>Chính thức</button>}
          {/* Bam hai lan moi cho nghi (khong dung confirm cua trinh duyet). */}
          <button onClick={() => (retire ? change("retired") : setRetire(true))}>{retire ? "Bấm lần nữa để cho nghỉ" : "Cho nghỉ"}</button>
        </div>
        <div className="set-actions">
          <button onClick={onEdit}>Sua</button>
          <button onClick={onLaunch}>Mo phien</button>
          <button className="primary" onClick={() => setForm({ title: "", body: "" })}>Giao viec</button>
        </div>
        {form && (
          <div className="assign">
            <input autoFocus placeholder="Tieu de task" value={form.title} onChange={(e) => setForm({ ...form, title: e.target.value })} />
            <textarea placeholder="Mo ta, tieu chi nghiem thu..." value={form.body} onChange={(e) => setForm({ ...form, body: e.target.value })} />
            <div className="set-actions">
              <button onClick={() => setForm(null)}>Huy</button>
              <button className="primary" disabled={!form.title.trim() || sending} onClick={submit}>Giao</button>
            </div>
          </div>
        )}
        <div className="ap-sub">Lich su task ({mine.length})</div>
        {mine.length === 0 && <div className="hint">Chua co task nao.</div>}
        {mine.map((t) => (
          <div key={t.id} className="ap-task">
            <span className={`bubble ${t.status}`}>{t.status === "done" ? (t.reason === "da huy" ? "Da huy" : "Da duyet") : bubble(t) || "Hang doi"}</span>
            <span className="t">{t.title}</span>
            <span className="d">{t.id}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
