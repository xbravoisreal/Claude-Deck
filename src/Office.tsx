import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { OfficePrefs } from "./Settings";
import { ThemeCtx, themeVars, type Theme } from "./theme";
import Room from "./Room";
import AgentPanel from "./AgentPanel";

export type AgentDef = { id: string; name: string; description: string; tools: string[]; model: string | null };
export type Dept = {
  id: string; name: string; icon: string; mission: string; workspace: string; agent_defs: AgentDef[]; errors: string[];
  room: unknown; status?: Record<string, string>; defaults?: { maxTokens?: number };
};
export type AgentLaunch = { label: string; args: string; settings: unknown };
export type Task = {
  id: string; dept: string; agent: string; title: string; status: "backlog" | "running" | "waiting" | "review" | "done";
  reason: string; session: string; dir: string; cwd: string; created: number; finished: number; started: boolean; summary: string;
};

/** Chu tren bong thoai theo trang thai task (tool: ten tool dang chay, neu biet). */
export const bubble = (t: Task, tool?: string) =>
  t.status === "review" ? "Chờ síp"
  : t.status === "waiting" ? `Chờ anh${t.reason ? `: ${t.reason}` : ""}`
  : t.status === "running" ? (tool ? `Dang: ${tool}` : `Dang: ${t.title}`)
  : "";

/** Man Office: luoi phong pixel-art. Moi lan mo lai man nay doc lai department/room.json va theme. */
export default function Office({ prefs, tasks, ctx, tools, onOpen, onTasks, onDepts, onHistory }: {
  prefs: OfficePrefs; tasks: Task[]; ctx: Record<string, number>; tools: Record<string, string>;
  onOpen: (cwd: string, agent: AgentLaunch) => void; onTasks: () => void; onDepts: (d: Dept[]) => void; onHistory: () => void;
}) {
  const [depts, setDepts] = useState<Dept[]>([]);
  const [theme, setTheme] = useState<Theme | null>(null);
  const [error, setError] = useState("");
  const [panel, setPanel] = useState<{ dept: string; agent: string } | null>(null);

  const reload = () =>
    invoke<Dept[]>("office_load", { root: prefs.root }).then((d) => (setDepts(d), onDepts(d)), (e) => setError(String(e)));
  useEffect(() => {
    if (!prefs.root) return;
    reload();
    invoke<Theme>("theme_load", { root: prefs.root, name: prefs.theme }).then(setTheme, (e) => setError(String(e)));
  }, [prefs.root, prefs.theme]);

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

  const assign = async (d: Dept, a: AgentDef, title: string, body: string) => {
    if (!d.workspace && !(await pickWorkspace(d))) return;
    await invoke("task_create", { root: prefs.root, dept: d.id, agent: a.id, title, body, notesRoot: prefs.notes || null }).catch((e) => {
      setError(String(e));
      throw e;
    });
    onTasks();
  };

  const reveal = (path: string) => invoke("open_in_editor", { path }).catch((e) => setError(String(e)));
  const pd = panel && depts.find((d) => d.id === panel.dept);
  const pa = pd?.agent_defs.find((a) => a.id === panel!.agent);

  if (!theme) return <div className="office">{error && <div className="office-error">{error}</div>}</div>;
  return (
    <ThemeCtx.Provider value={theme}>
      <div className="office" style={themeVars(theme)}>
        {error && <div className="office-error" onClick={() => setError("")}>{error}</div>}
        {theme.errors.map((e) => <div key={e} className="office-error">theme: {e}</div>)}
        {depts.map((d) => (
          <div key={d.id} className={`room${d.agent_defs.length > 3 ? " wide" : ""}`}>
            <div className="room-head">
              <b>{d.icon} {d.name}</b>
              <button className="room-ws" title="Doi thu muc du an" onClick={() => safe(() => pickWorkspace(d))}>{d.workspace || "Chon thu muc du an"}</button>
            </div>
            <div className="room-mission">{d.mission}</div>
            {d.errors.map((e) => <div key={e} className="office-error">{e}</div>)}
            <Room dept={d} tasks={tasks} ctx={ctx} tools={tools} warnK={prefs.warnK} onDesk={(a) => setPanel({ dept: d.id, agent: a.id })} onHistory={onHistory} />
          </div>
        ))}
        <button className="room room-new" title="Them thu muc trong departments/ (xem themes/README.md)" onClick={() => reveal(`${prefs.root}\\departments`)}>
          + Mo phong
        </button>
        {pd && pa && (
          <AgentPanel
            key={`${pd.id}/${pa.id}`}
            dept={pd}
            agent={pa}
            tasks={tasks}
            onAssign={(title, body) => assign(pd, pa, title, body)}
            onLaunch={() => launch(pd, pa)}
            onEdit={() => reveal(`${prefs.root}\\departments\\${pd.id}\\agents\\${pa.id}.md`)}
            onClose={() => setPanel(null)}
          />
        )}
      </div>
    </ThemeCtx.Provider>
  );
}
