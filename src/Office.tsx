import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { OfficePrefs } from "./Settings";
import { ThemeCtx, themeVars, type Theme } from "./theme";
import Room from "./Room";
import AgentPanel from "./AgentPanel";
import Candidates from "./Candidates";
import Stats from "./Stats";

export type AgentDef = { id: string; name: string; description: string; tools: string[]; model: string | null };
export type Dept = {
  id: string; name: string; icon: string; mission: string; workspace: string; agent_defs: AgentDef[]; errors: string[];
  room: unknown; status?: Record<string, string>; budget?: Record<string, number>; defaults?: { maxTokens?: number };
};
export type Template = { id: string; name: string; icon: string; mission: string; agents: string[] };
export type AgentLaunch = { label: string; args: string; settings: unknown };
export type Task = {
  id: string; dept: string; agent: string; title: string; status: "backlog" | "running" | "waiting" | "review" | "done";
  reason: string; session: string; dir: string; cwd: string; created: number; finished: number; started: boolean; summary: string; ask: string; model: string;
};

/** Chu tren bong thoai theo trang thai task (tool: ten tool dang chay, neu biet). */
export const bubble = (t: Task, tool?: string) =>
  t.status === "review" ? "Chờ síp duyệt"
  : t.status === "waiting" ? `Chờ síp trả lời${t.reason ? `: ${t.reason}` : ""}`
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
  const [newRoom, setNewRoom] = useState<{ name: string; icon: string; mission: string; template: string; askHr: boolean } | null>(null);
  const [templates, setTemplates] = useState<Template[]>([]);
  const [stats, setStats] = useState(false);
  const [hire, setHire] = useState<{ dept: Dept; need: string } | null>(null);

  const reload = () =>
    invoke<Dept[]>("office_load", { root: prefs.root }).then((d) => (setDepts(d), onDepts(d)), (e) => setError(String(e)));
  useEffect(() => {
    if (!prefs.root) return;
    reload();
    invoke<Theme>("theme_load", { root: prefs.root, name: prefs.theme }).then(setTheme, (e) => setError(String(e)));
  }, [prefs.root, prefs.theme]);
  // Bot Telegram doi du an cua phong (/duan) -> ve lai ke sach.
  useEffect(() => {
    const off = listen("office-depts-changed", () => prefs.root && reload());
    return () => void off.then((f) => f());
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

  const assign = async (d: Dept, a: AgentDef, title: string, body: string, model: string) => {
    if (!d.workspace && !(await pickWorkspace(d))) return;
    await invoke("task_create", { root: prefs.root, dept: d.id, agent: a.id, title, body, notesRoot: prefs.notes || null, model: model || null }).catch((e) => {
      setError(String(e));
      throw e;
    });
    onTasks();
  };

  const createRoom = () =>
    safe(async () => {
      if (!newRoom?.name.trim()) return;
      const { name, icon, mission, template, askHr } = newRoom;
      const id = await invoke<string>("office_create_department", { root: prefs.root, name, icon, mission, template: template || null });
      setNewRoom(null);
      reload();
      // Phong trong + nho HR: Recruiter de xuat doi hinh, nop moi Agent mot ho so; nguoi dau tien duoc nhan lam lead.
      if (!template && askHr) {
        const need = `Dựng đội hình cho phòng mới "${name.trim()}". Nhiệm vụ: ${mission.trim() || "(hỏi Síp)"}.
Đề xuất 3–5 Agent, một Agent một việc, nộp mỗi Agent một hồ sơ ứng viên. Agent đầu tiên Síp nhận sẽ là trưởng phòng (điều phối, có tool Agent để gọi các Agent khác): ghi rõ trong reason hồ sơ nào nên nhận trước.`;
        await invoke("hr_request", { root: prefs.root, dept: id, need, notesRoot: prefs.notes || null });
        onTasks();
      }
    });
  const openNewRoom = () => {
    if (newRoom) return setNewRoom(null);
    setNewRoom({ name: "", icon: "🏢", mission: "", template: "", askHr: false });
    invoke<Template[]>("office_templates", { root: prefs.root }).then(setTemplates, (e) => setError(String(e)));
  };
  const pickTemplate = (id: string) => {
    const t = templates.find((t) => t.id === id);
    setNewRoom((r) => r && (t ? { ...r, template: id, icon: t.icon, name: r.name || t.name, mission: r.mission || t.mission } : { ...r, template: "" }));
  };

  // "+ Tuyen": giao yeu cau cho Recruiter phong HR.
  const recruit = () =>
    safe(async () => {
      if (!hire?.need.trim()) return;
      await invoke("hr_request", { root: prefs.root, dept: hire.dept.id, need: hire.need.trim(), notesRoot: prefs.notes || null });
      setHire(null);
      reload();
      onTasks();
    });
  const setStatus = async (d: Dept, a: AgentDef, status: string) => {
    await invoke("office_set_agent_status", { root: prefs.root, dept: d.id, agent: a.id, status });
    if (status === "retired") setPanel(null);
    reload();
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
            {d.errors.map((e) => <div key={e} className="office-error room-err">{e}</div>)}
            <Room
              dept={d}
              tasks={tasks}
              ctx={ctx}
              tools={tools}
              warnK={prefs.warnK}
              onDesk={(a) => setPanel({ dept: d.id, agent: a.id })}
              onHistory={onHistory}
              onWorkspace={() => safe(() => pickWorkspace(d))}
              onHire={() => setHire({ dept: d, need: "" })}
            />
            {d.id === "hr" && <Candidates root={prefs.root} notes={prefs.notes} depts={depts} tick={tasks} onTasks={onTasks} onHired={reload} />}
          </div>
        ))}
        {/* Nut + tron goc duoi trai: tao phong moi. */}
        {newRoom && <div className="drawer-backdrop" onClick={() => setNewRoom(null)} />}
        {newRoom && (
          <div className="new-room">
            <b>Mo phong moi</b>
            <select value={newRoom.template} onChange={(e) => pickTemplate(e.target.value)}>
              <option value="">Phòng trống</option>
              {templates.map((t) => <option key={t.id} value={t.id}>{t.icon} {t.name} · {t.agents.length} Agent</option>)}
            </select>
            <div className="new-room-row">
              <input className="new-room-icon" value={newRoom.icon} onChange={(e) => setNewRoom({ ...newRoom, icon: e.target.value })} />
              <input autoFocus placeholder="Ten phong (vd Content Media)" value={newRoom.name} onChange={(e) => setNewRoom({ ...newRoom, name: e.target.value })} onKeyDown={(e) => e.key === "Enter" && createRoom()} />
            </div>
            <input placeholder="Nhiem vu cua phong" value={newRoom.mission} onChange={(e) => setNewRoom({ ...newRoom, mission: e.target.value })} onKeyDown={(e) => e.key === "Enter" && createRoom()} />
            {newRoom.template ? (
              <div className="hint">Chép đội hình từ template: {templates.find((t) => t.id === newRoom.template)?.agents.join(", ")}</div>
            ) : (
              <label className="hint"><input type="checkbox" checked={newRoom.askHr} onChange={(e) => setNewRoom({ ...newRoom, askHr: e.target.checked })} /> Nhờ HR đề xuất đội hình (theo nhiệm vụ)</label>
            )}
            <div className="set-actions">
              <button onClick={() => reveal(`${prefs.root}\\templates\\departments`)}>Mo template</button>
              <button onClick={() => reveal(`${prefs.root}\\departments`)}>Mo thu muc</button>
              <button onClick={() => setNewRoom(null)}>Huy</button>
              <button className="primary" disabled={!newRoom.name.trim()} onClick={createRoom}>Tao</button>
            </div>
          </div>
        )}
        {hire && <div className="drawer-backdrop" onClick={() => setHire(null)} />}
        {hire && (
          <div className="new-room">
            <b>Tuyển cho {hire.dept.icon} {hire.dept.name}</b>
            <textarea autoFocus placeholder="Cần Agent làm gì? (vd viết changelog từ git log mỗi lần release)" value={hire.need} onChange={(e) => setHire({ ...hire, need: e.target.value })} />
            <div className="hint">Recruiter (phòng HR) sẽ hỏi lại nếu cần, rồi soạn hồ sơ ở khu Ứng viên chờ duyệt.</div>
            <div className="set-actions">
              <button onClick={() => setHire(null)}>Huy</button>
              <button className="primary" disabled={!hire.need.trim()} onClick={recruit}>Gửi HR</button>
            </div>
          </div>
        )}
        <button className="room-add" title="Mo phong moi" onClick={openNewRoom}>+</button>
        <button className="room-add room-stats" title="Thống kê Agent" onClick={() => setStats(!stats)}>📊</button>
        {stats && <div className="drawer-backdrop" onClick={() => setStats(false)} />}
        {stats && <Stats root={prefs.root} notes={prefs.notes} depts={depts} onTasks={onTasks} onClose={() => setStats(false)} />}
        {pd && pa && <div className="drawer-backdrop" onClick={() => setPanel(null)} />}
        {pd && pa && (
          <AgentPanel
            key={`${pd.id}/${pa.id}`}
            root={prefs.root}
            notes={prefs.notes}
            onTasks={onTasks}
            dept={pd}
            agent={pa}
            tasks={tasks}
            onAssign={(title, body, model) => assign(pd, pa, title, body, model)}
            onLaunch={() => launch(pd, pa)}
            onEdit={() => reveal(`${prefs.root}\\departments\\${pd.id}\\agents\\${pa.id}.md`)}
            onStatus={(s) => setStatus(pd, pa, s)}
            onClose={() => setPanel(null)}
          />
        )}
      </div>
    </ThemeCtx.Provider>
  );
}
