import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { bubble, type Task } from "./Office";

const when = (ms: number) => new Date(ms).toLocaleString();
const ORDER = ["review", "waiting", "running", "backlog", "done"] as const;
const LABEL: Record<string, string> = { review: "Chờ duyệt", waiting: "Chờ trả lời", running: "Đang làm", backlog: "Hàng đợi", done: "Xong" };

/** Tab Task: moi task cua moi phong, loc theo trang thai. Lam lai (can nhan xet) xu ly o ☰. */
export function TaskBoard({ tasks, names, onApprove, onCancel, onOpen }: {
  tasks: Task[]; names: Record<string, string>; onApprove: (t: Task) => void; onCancel: (t: Task) => void; onOpen: (t: Task) => void;
}) {
  const [filter, setFilter] = useState<string>("");
  const list = tasks.filter((t) => !filter || t.status === filter).sort((a, b) => ORDER.indexOf(a.status) - ORDER.indexOf(b.status) || (b.finished || b.created) - (a.finished || a.created));
  return (
    <div className="board">
      <div className="board-bar">
        {["", ...ORDER].map((s) => (
          <button key={s} className={filter === s ? "on" : ""} onClick={() => setFilter(s)}>
            {s ? LABEL[s] : "Tất cả"} ({tasks.filter((t) => !s || t.status === s).length})
          </button>
        ))}
      </div>
      {list.length === 0 && <div className="hint">Không có task.</div>}
      {list.map((t) => (
        <div key={t.id} className="board-row">
          <span className={`bubble ${t.status === "done" ? (t.reason === "da huy" ? "waiting" : "done") : t.status}`}>{t.status === "done" ? (t.reason === "da huy" ? "Đã hủy" : "Đã duyệt") : bubble(t) || LABEL[t.status]}</span>
          <span className="t">{t.title}</span>
          <span className="d">{names[`${t.dept}/${t.agent}`] ?? t.agent}{t.model && ` · ${t.model}`} · {when(t.finished || t.created)} · {t.id}</span>
          {t.status !== "done" && t.status !== "backlog" && <button onClick={() => onOpen(t)}>Mở phiên</button>}
          {(t.status === "waiting" || t.status === "backlog") && <button onClick={() => onCancel(t)}>Hủy</button>}
          {t.status === "review" && <button className="primary" onClick={() => onApprove(t)}>Duyệt</button>}
        </div>
      ))}
    </div>
  );
}

type Routine = { id: string; dept: string; agent: string; title: string; prompt: string; at: string; days: number[]; enabled: boolean; last: number; task: string };
const DAYS = ["T2", "T3", "T4", "T5", "T6", "T7", "CN"];

/** Tab Routine: moi routine cua moi phong. Them/sua noi dung o panel cua Agent (bam vao ban). */
export function RoutineBoard({ root, notes, names, onTasks }: { root: string; notes: string; names: Record<string, string>; onTasks: () => void }) {
  const [rs, setRs] = useState<Routine[]>([]);
  const [error, setError] = useState("");
  const load = () => invoke<Routine[]>("routine_list", { root }).then(setRs, (e) => setError(String(e)));
  useEffect(() => void load(), [root]);
  const run = (cmd: string, args: object) => invoke(cmd, { root, ...args }).then(() => (load(), onTasks()), (e) => setError(String(e)));
  return (
    <div className="board">
      {error && <div className="office-error" onClick={() => setError("")}>{error}</div>}
      {rs.length === 0 && <div className="hint">Chưa có routine nào. Tạo ở panel của Agent (bấm vào bàn).</div>}
      {rs.map((r) => (
        <div key={`${r.dept}/${r.id}`} className={`board-row${r.enabled ? "" : " off"}`}>
          <input type="checkbox" title="Bật/tắt" checked={r.enabled} onChange={() => run("routine_save", { routine: { ...r, enabled: !r.enabled } })} />
          <span className="t" title={r.prompt}>{r.title}</span>
          <span className="d">{names[`${r.dept}/${r.agent}`] ?? r.agent} · {r.at} {r.days.length === 0 || r.days.length === 7 ? "mỗi ngày" : r.days.map((d) => DAYS[d - 1]).join(" ")}{r.last ? ` · lần cuối ${when(r.last)}` : ""}</span>
          <button onClick={() => run("routine_run", { dept: r.dept, id: r.id, notesRoot: notes || null })}>Chạy ngay</button>
          <button onClick={() => run("routine_delete", { dept: r.dept, id: r.id })}>Xóa</button>
        </div>
      ))}
    </div>
  );
}
