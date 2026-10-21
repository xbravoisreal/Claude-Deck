import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { OfficePrefs } from "./Settings";

type AgentDef = { id: string; name: string; description: string; tools: string[]; model: string | null };
export type Dept = { id: string; name: string; icon: string; mission: string; workspace: string; agent_defs: AgentDef[]; errors: string[] };
export type AgentLaunch = { label: string; args: string; settings: unknown };
export type Task = {
  id: string; dept: string; agent: string; title: string; status: "backlog" | "running" | "waiting" | "review" | "done";
  reason: string; session: string; dir: string; cwd: string; created: number; started: boolean; summary: string;
};

/** Chu tren bong thoai theo trang thai task. */
export const bubble = (t: Task) =>
  t.status === "review" ? "Chờ síp" : t.status === "waiting" ? `Chờ anh${t.reason ? `: ${t.reason}` : ""}` : t.status === "running" ? `Dang: ${t.title}` : "";

// ponytail: danh sach don gian, giao dien pixel-art la Giai doan 3.
export default function Office({ prefs, tasks, ctx, onOpen, onTasks, onDepts }: {
  prefs: OfficePrefs; tasks: Task[]; ctx: Record<string, number>;
  onOpen: (cwd: string, agent: AgentLaunch) => void; onTasks: () => void; onDepts: (d: Dept[]) => void;
}) {
  const [depts, setDepts] = useState<Dept[]>([]);
  const [error, setError] = useState("");
  const [assign, setAssign] = useState<{ dept: Dept; agent: AgentDef; title: string; body: string } | null>(null);

  const reload = () =>
    invoke<Dept[]>("office_load", { root: prefs.root }).then((d) => (setDepts(d), onDepts(d)), (e) => setError(String(e)));
  useEffect(() => {
    if (prefs.root) reload();
  }, [prefs.root]);

  const pickWorkspace = async (d: Dept) => {
    const dir = await open({ directory: true });
    if (typeof dir !== "string") return null;
    await invoke("office_set_workspace", { root: prefs.root, dept: d.id, workspace: dir });
    reload();
    return dir;
  };
  const safe = (f: () => Promise<unknown>) => f().catch((e) => setError(String(e)));

  const launch = (d: Dept, a: AgentDef) =>
    safe(async () => {
      const cwd = d.workspace || (await pickWorkspace(d));
      if (!cwd) return;
      const spec = await invoke<{ args: string; settings: unknown }>("office_launch", { root: prefs.root, dept: d.id, agent: a.id, notesRoot: prefs.notes || null });
      onOpen(cwd, { label: `${d.icon} ${a.name}`, ...spec });
    });

  const submit = () =>
    safe(async () => {
      if (!assign || !assign.title.trim()) return;
      if (!assign.dept.workspace && !(await pickWorkspace(assign.dept))) return;
      await invoke("task_create", { root: prefs.root, dept: assign.dept.id, agent: assign.agent.id, title: assign.title.trim(), body: assign.body, notesRoot: prefs.notes || null });
      setAssign(null);
      onTasks();
    });

  return (
    <div className="office">
      {error && <div className="office-error" onClick={() => setError("")}>{error}</div>}
      {depts.map((d) => (
        <div key={d.id} className="room">
          <div className="room-head">
            <b>{d.icon} {d.name}</b>
            <button className="room-ws" title="Doi thu muc du an" onClick={() => safe(() => pickWorkspace(d))}>{d.workspace || "Chon thu muc du an"}</button>
          </div>
          <div className="room-mission">{d.mission}</div>
          {d.errors.map((e) => <div key={e} className="office-error">{e}</div>)}
          {d.agent_defs.map((a) => {
            const mine = tasks.filter((t) => t.dept === d.id && t.agent === a.id);
            const cur = mine.find((t) => t.status === "running" || t.status === "waiting") ?? mine.find((t) => t.status === "review");
            const queued = mine.filter((t) => t.status === "backlog").length;
            const warn = cur && (ctx[cur.id] ?? 0) >= prefs.warnK * 1000;
            return (
              <div key={a.id}>
                <div className={`agent-row${warn ? " warn" : ""}`} title={warn ? "phien dai, nen tach task" : a.description}>
                  <span className="agent-name">{a.name}</span>
                  <span className="agent-meta">
                    {cur ? <span className={`bubble ${cur.status}`}>{bubble(cur)}</span> : `${a.model ?? "default"} · ${a.tools.join(", ")}`}
                    {queued > 0 && <span className="queued"> · hang doi {queued}</span>}
                  </span>
                  <button onClick={() => setAssign({ dept: d, agent: a, title: "", body: "" })}>Giao viec</button>
                  <button onClick={() => launch(d, a)}>Mo phien</button>
                </div>
                {assign?.dept.id === d.id && assign.agent.id === a.id && (
                  <div className="assign">
                    <input autoFocus placeholder="Tieu de task" value={assign.title} onChange={(e) => setAssign({ ...assign, title: e.target.value })} />
                    <textarea placeholder="Mo ta, tieu chi nghiem thu..." value={assign.body} onChange={(e) => setAssign({ ...assign, body: e.target.value })} />
                    <div className="set-actions">
                      <button onClick={() => setAssign(null)}>Huy</button>
                      <button className="primary" disabled={!assign.title.trim()} onClick={submit}>Giao</button>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      ))}
    </div>
  );
}
