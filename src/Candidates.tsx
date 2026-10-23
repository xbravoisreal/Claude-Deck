import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AgentDef, Dept } from "./Office";

type Probation = { ok: boolean; result: string; turns: number; tokens: number; cost: number; files: string[]; when: number; denied: string[] };
type Candidate = { id: string; dept: string; reason: string; agent: AgentDef | null; sample: string; probation: Probation | null; error: string };

/** Khu "Ung vien cho duyet" trong phong HR: Thu viec / Nhan / Sua / Tu choi. */
export default function Candidates({ root, notes, depts, tick, onTasks, onHired }: {
  root: string; notes: string; depts: Dept[]; tick: unknown; onTasks: () => void; onHired: () => void;
}) {
  const [list, setList] = useState<Candidate[]>([]);
  const [busy, setBusy] = useState<Record<string, string>>({});
  const [note, setNote] = useState<{ id: string; text: string } | null>(null);
  const [error, setError] = useState("");

  const load = () => invoke<Candidate[]>("hr_candidates", { root }).then(setList, (e) => setError(String(e)));
  // tick: danh sach task doi (Recruiter vua xong) -> doc lai ho so.
  useEffect(() => void load(), [root, tick]);

  const run = async (id: string, what: string, f: () => Promise<unknown>) => {
    setBusy((b) => ({ ...b, [id]: what }));
    setError("");
    try {
      await f();
    } catch (e) {
      setError(`${id}: ${e}`);
    } finally {
      setBusy(({ [id]: _, ...b }) => b);
      load();
    }
  };
  const deptName = (id: string) => depts.find((d) => d.id === id)?.name ?? id;

  return (
    <div className="candidates">
      <div className="cand-head">Ứng viên chờ duyệt ({list.length})</div>
      {error && <div className="office-error" onClick={() => setError("")}>{error}</div>}
      {list.length === 0 && <div className="hint">Chưa có. Bấm "+ Tuyển" ở một phòng để Recruiter soạn hồ sơ.</div>}
      {list.map((c) => {
        const p = c.probation;
        const b = busy[c.id];
        return (
          <div key={c.id} className="cand">
            <div className="cand-title">
              <b>{c.agent?.name ?? c.id}</b> → {deptName(c.dept)} <span className="d">{c.id}</span>
            </div>
            {c.agent && <div className="cand-desc">{c.agent.description}</div>}
            {c.reason && <div className="cand-desc">Lý do: {c.reason}</div>}
            {c.agent && (
              <div className="cand-tools">
                {(c.agent.tools.length ? c.agent.tools : ["kế thừa mọi tool"]).map((t) => <span key={t} className="chip">{t}</span>)}
                {c.agent.model && <span className="chip">{c.agent.model}</span>}
              </div>
            )}
            {c.error && <div className="office-error">{c.error}</div>}
            <div className={`cand-prob${p ? (p.ok ? " ok" : " bad") : ""}`}>
              {b === "probation" ? "Đang thử việc… (tối đa 10 phút)"
                : !p ? "Chưa thử việc"
                : <>
                    <span>{p.ok ? "✅ Thử việc xong" : "⚠️ Thử việc lỗi"} · {p.turns} lượt · {Math.round(p.tokens / 1000)}K token · ${p.cost.toFixed(3)}</span>
                    <span className="cand-result">{p.result}</span>
                    {p.files.length > 0 && <span className="d">File tạo ra: {p.files.join(", ")}</span>}
                    {p.denied.length > 0 && <span className="d">Bị chặn: {p.denied.join(", ")}</span>}
                  </>}
            </div>
            {note?.id === c.id ? (
              <div className="assign">
                <textarea autoFocus placeholder="Cần sửa gì?" value={note.text} onChange={(e) => setNote({ id: c.id, text: e.target.value })} />
                <div className="set-actions">
                  <button onClick={() => setNote(null)}>Hủy</button>
                  <button className="primary" disabled={!note.text.trim()} onClick={() => (setNote(null), run(c.id, "revise", () => invoke("hr_revise", { root, cid: c.id, note: note.text, notesRoot: notes || null }).then(onTasks)))}>Gửi Recruiter</button>
                </div>
              </div>
            ) : (
              <div className="set-actions">
                <button disabled={!!b || !c.agent} onClick={() => run(c.id, "probation", () => invoke("hr_probation", { root, cid: c.id }))}>Thử việc</button>
                <button disabled={!!b} onClick={() => setNote({ id: c.id, text: "" })}>Sửa</button>
                <button disabled={!!b} onClick={() => run(c.id, "reject", () => invoke("hr_reject", { root, cid: c.id }))}>Từ chối</button>
                <button className="primary" disabled={!!b || !!c.error} onClick={() => run(c.id, "hire", () => invoke("hr_hire", { root, cid: c.id }).then(onHired))}>Nhận</button>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
