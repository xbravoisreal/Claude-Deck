import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { bubble, type AgentDef, type Dept, type Task } from "./Office";

/** Panel chi tiet Agent (bam vao ban): thong tin, lich su task, Giao viec / Mo phien / Sua. */
const STATUS: Record<string, string> = { active: "Chính thức", probation: "Thử việc", retired: "Đã nghỉ" };
const DAYS = ["T2", "T3", "T4", "T5", "T6", "T7", "CN"];

/** Routine: den gio app tu giao task cho Agent (luu o departments/<phong>/routines.json). */
type Routine = { id: string; dept: string; agent: string; title: string; prompt: string; at: string; days: number[]; enabled: boolean; last: number; task: string };
const when = (r: Routine) => `${r.at} ${r.days.length === 0 || r.days.length === 7 ? "mỗi ngày" : r.days.map((d) => DAYS[d - 1]).join(" ")}`;

export default function AgentPanel({ root, notes, dept, agent, tasks, onAssign, onLaunch, onEdit, onStatus, onTasks, onClose }: {
  root: string; notes: string; dept: Dept; agent: AgentDef; tasks: Task[]; onTasks: () => void;
  onAssign: (title: string, body: string, model: string, reference: string) => Promise<unknown>; onLaunch: () => void; onEdit: () => void;
  onStatus: (status: string) => Promise<unknown>; onClose: () => void;
}) {
  const [retire, setRetire] = useState(false);
  const [error, setError] = useState("");
  const status = dept.status?.[agent.id] ?? "active";
  const change = (s: string) => onStatus(s).catch((e) => (setError(String(e)), setRetire(false)));
  const [form, setForm] = useState<{ title: string; body: string; model: string; ref: string; images: string[] } | null>(null);
  const [sending, setSending] = useState(false);
  const mine = tasks.filter((t) => t.dept === dept.id && t.agent === agent.id).sort((a, b) => b.created - a.created);
  // Tham chieu: task da duyet cua phong co worktree rieng, de mo ra lam tiep.
  const refs = tasks.filter((t) => t.dept === dept.id && t.status === "done" && t.reason !== "da huy" && /[\\/]\.office[\\/]worktrees[\\/]/.test(t.cwd)).sort((a, b) => b.finished - a.finished);
  const addImages = (paths: string[]) => setForm((f) => f && { ...f, images: [...f.images, ...paths] });
  const pasteImage = async (e: React.ClipboardEvent) => {
    const file = [...e.clipboardData.items].find((i) => i.type.startsWith("image/"))?.getAsFile();
    if (!file) return;
    e.preventDefault();
    addImages([await invoke<string>("save_image", new Uint8Array(await file.arrayBuffer()))]);
  };
  const pickImages = async () => {
    const r = await open({ multiple: true, filters: [{ name: "Anh", extensions: ["png", "jpg", "jpeg", "gif", "webp"] }] });
    if (Array.isArray(r)) addImages(r);
  };
  const budget = dept.budget?.[agent.id] ?? dept.defaults?.maxTokens ?? 4_000_000;

  const [routines, setRoutines] = useState<Routine[]>([]);
  const [rform, setRform] = useState<Routine | null>(null);
  const loadRoutines = () =>
    invoke<Routine[]>("routine_list", { root }).then((rs) => setRoutines(rs.filter((r) => r.dept === dept.id && r.agent === agent.id)), (e) => setError(String(e)));
  useEffect(() => void loadRoutines(), [root, dept.id, agent.id]);
  const routine = (cmd: string, args: object) => invoke(cmd, { root, ...args }).then(loadRoutines, (e) => setError(String(e)));
  const blank: Routine = { id: "", dept: dept.id, agent: agent.id, title: "", prompt: "", at: "08:00", days: [], enabled: true, last: 0, task: "" };

  const submit = async () => {
    if (!form?.title.trim() || sending) return;
    setSending(true); // bam 2 lan khong tao 2 task
    try {
      // Anh di kem la duong dan file: Agent tu doc anh bang tool Read.
      const imgs = form.images.length ? `\n\nAnh dinh kem (doc bang Read):\n${form.images.map((p) => `- ${p}`).join("\n")}` : "";
      await onAssign(form.title.trim(), form.body + imgs, form.model, form.ref);
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
          <button className="primary" onClick={() => setForm({ title: "", body: "", model: "", ref: "", images: [] })}>Giao viec</button>
        </div>
        {form && (
          <div className="assign">
            <input autoFocus placeholder="Tieu de task" value={form.title} onChange={(e) => setForm({ ...form, title: e.target.value })} />
            <textarea placeholder="Mo ta, tieu chi nghiem thu... (dan anh vao day duoc)" value={form.body} onChange={(e) => setForm({ ...form, body: e.target.value })} onPaste={pasteImage} />
            {form.images.map((p) => (
              <div key={p} className="hint" title={p}>🖼 {p.split(/[\\/]/).pop()} <button onClick={() => setForm({ ...form, images: form.images.filter((x) => x !== p) })}>✕</button></div>
            ))}
            <button onClick={pickImages}>Dinh kem anh...</button>
            <select value={form.ref} title="Mo ra lam tiep tu task da duyet" onChange={(e) => setForm({ ...form, ref: e.target.value })}>
              <option value="">Tham chiếu: không</option>
              {refs.map((t) => <option key={t.id} value={t.id}>{t.id}-{t.agent} · {t.title}</option>)}
            </select>
            <select value={form.model} title="Tự động: theo router của phòng, không khớp thì model của Agent" onChange={(e) => setForm({ ...form, model: e.target.value })}>
              <option value="">Model: tự động ({agent.model ?? "default"})</option>
              {["haiku", "sonnet", "opus"].map((m) => <option key={m} value={m}>{m}</option>)}
            </select>
            <div className="set-actions">
              <button onClick={() => setForm(null)}>Huy</button>
              <button className="primary" disabled={!form.title.trim() || sending} onClick={submit}>Giao</button>
            </div>
          </div>
        )}
        <div className="ap-sub">
          Routine ({routines.length}) {!rform && status !== "retired" && <button onClick={() => setRform(blank)}>+ Them</button>}
        </div>
        {routines.map((r) => (
          <div key={r.id} className={`ap-routine${r.enabled ? "" : " off"}`}>
            <input type="checkbox" title="Bat/tat" checked={r.enabled} onChange={() => routine("routine_save", { routine: { ...r, enabled: !r.enabled } })} />
            <span className="t" title={r.prompt}>{r.title}</span>
            <span className="d">{when(r)}{r.last ? ` · lần cuối ${new Date(r.last).toLocaleString()}` : ""}</span>
            <button onClick={() => setRform(r)}>Sua</button>
            <button onClick={() => invoke("routine_run", { root, dept: dept.id, id: r.id, notesRoot: notes || null }).then(() => (loadRoutines(), onTasks()), (e) => setError(String(e)))}>Chay ngay</button>
            <button onClick={() => routine("routine_delete", { dept: dept.id, id: r.id })}>Xoa</button>
          </div>
        ))}
        {rform && (
          <div className="assign">
            <input autoFocus placeholder="Ten routine (vd Bao cao loi moi sang)" value={rform.title} onChange={(e) => setRform({ ...rform, title: e.target.value })} />
            <textarea placeholder="Viec can lam moi lan chay..." value={rform.prompt} onChange={(e) => setRform({ ...rform, prompt: e.target.value })} />
            <div className="ap-days">
              <input type="time" value={rform.at} onChange={(e) => setRform({ ...rform, at: e.target.value })} />
              {DAYS.map((d, i) => (
                <label key={d}>
                  <input type="checkbox" checked={rform.days.includes(i + 1)} onChange={(e) => setRform({ ...rform, days: e.target.checked ? [...rform.days, i + 1].sort() : rform.days.filter((x) => x !== i + 1) })} /> {d}
                </label>
              ))}
            </div>
            <div className="hint">Không chọn ngày = mỗi ngày. Đến giờ app tự giao task (app phải đang mở, kể cả dưới khay).</div>
            <div className="set-actions">
              <button onClick={() => setRform(null)}>Huy</button>
              <button className="primary" disabled={!rform.title.trim()} onClick={() => routine("routine_save", { routine: { ...rform, title: rform.title.trim() } }).then(() => setRform(null))}>Luu</button>
            </div>
          </div>
        )}
        <div className="ap-sub">Lich su task ({mine.length})</div>
        {mine.length === 0 && <div className="hint">Chua co task nao.</div>}
        {mine.map((t) => (
          <div key={t.id} className="ap-task">
            <span className={`bubble ${t.status}`}>{t.status === "done" ? (t.reason === "da huy" ? "Da huy" : "Da duyet") : bubble(t) || "Hang doi"}</span>
            <span className="t">{t.title}</span>
            <span className="d">{t.model && `${t.model} · `}{t.id}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
