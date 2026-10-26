import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Dept } from "./Office";

type Usage = { input: number; output: number; cache_read: number; cache_create: number; turns: number };
type Row = { dept: string; agent: string; tasks: number; approved: number; cancelled: number; open: number; reworks: number; total: Usage; by_model: Record<string, Usage> };

const k = (n: number) => (n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : `${Math.round(n / 1000)}K`);
/** "claude-sonnet-5-5" -> "sonnet" */
const short = (m: string) => m.replace(/^claude-/, "").replace(/-[\d-]+$/, "");

/** Bang thong ke token theo phong / Agent / model, va nut gui HR danh gia. Doc transcript khi mo (co the mat vai giay). */
export default function Stats({ root, notes, depts, onTasks, onClose }: { root: string; notes: string; depts: Dept[]; onTasks: () => void; onClose: () => void }) {
  const [rows, setRows] = useState<Row[] | null>(null);
  const [msg, setMsg] = useState("");
  useEffect(() => {
    invoke<Row[]>("office_stats", { root }).then(setRows, (e) => setMsg(String(e)));
  }, [root]);
  const name = (d: string, a: string) => depts.find((x) => x.id === d)?.agent_defs.find((x) => x.id === a)?.name ?? a;
  const review = () =>
    invoke("hr_review", { root, notesRoot: notes || null }).then(() => (setMsg("Đã giao Recruiter đánh giá."), onTasks()), (e) => setMsg(String(e)));

  return (
    <div className="history agent-panel stats-panel">
      <div className="history-head">
        Thống kê Agent <button onClick={onClose}>✕</button>
      </div>
      <div className="history-list">
        {msg && <div className="hint">{msg}</div>}
        {!rows && !msg && <div className="hint">Đang đọc transcript...</div>}
        {rows?.length === 0 && <div className="hint">Chưa có task nào.</div>}
        {depts.filter((d) => rows?.some((r) => r.dept === d.id)).map((d) => (
          <div key={d.id}>
            <div className="ap-sub">{d.icon} {d.name}</div>
            <table className="stats">
              <thead>
                <tr><th>Agent</th><th title="Duyệt / Hủy / Đang mở">Task</th><th>Làm lại</th><th>Lượt</th><th title="Token đọc cache (phần tốn usage nhất)">Đọc cache</th><th>Output</th><th>TB/task</th><th>Theo model</th></tr>
              </thead>
              <tbody>
                {rows!.filter((r) => r.dept === d.id).map((r) => (
                  <tr key={r.agent}>
                    <td>{name(r.dept, r.agent)}</td>
                    <td>{r.approved}/{r.cancelled}/{r.open}</td>
                    <td>{r.reworks}</td>
                    <td>{r.total.turns}</td>
                    <td>{k(r.total.cache_read)}</td>
                    <td>{k(r.total.output)}</td>
                    <td>{r.tasks ? k(r.total.cache_read / r.tasks) : "-"}</td>
                    <td>{Object.entries(r.by_model).map(([m, u]) => `${short(m)} ${k(u.cache_read)}`).join(", ")}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ))}
        <div className="hint">Token của subagent tính cho Agent làm subagent (vd Dev do Tech Lead gọi). TB/task chia theo task được giao trực tiếp.</div>
        <div className="set-actions">
          <button className="primary" disabled={!rows?.length} onClick={review}>HR đánh giá</button>
        </div>
      </div>
    </div>
  );
}
