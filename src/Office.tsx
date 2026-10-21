import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { OfficePrefs } from "./Settings";

type AgentDef = { id: string; name: string; description: string; tools: string[]; model: string | null };
type Dept = { id: string; name: string; icon: string; mission: string; workspace: string; agent_defs: AgentDef[]; errors: string[] };
export type AgentLaunch = { label: string; args: string; settings: unknown };

// ponytail: danh sach don gian, giao dien pixel-art la Giai doan 3.
export default function Office({ prefs, onOpen }: { prefs: OfficePrefs; onOpen: (cwd: string, agent: AgentLaunch) => void }) {
  const [depts, setDepts] = useState<Dept[]>([]);
  const [error, setError] = useState("");

  const reload = () =>
    invoke<Dept[]>("office_load", { root: prefs.root }).then(setDepts, (e) => setError(String(e)));
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

  const launch = async (d: Dept, a: AgentDef) => {
    try {
      const cwd = d.workspace || (await pickWorkspace(d));
      if (!cwd) return;
      const spec = await invoke<{ args: string; settings: unknown }>("office_launch", { root: prefs.root, dept: d.id, agent: a.id, notesRoot: prefs.notes || null });
      onOpen(cwd, { label: `${d.icon} ${a.name}`, ...spec });
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="office">
      {error && <div className="office-error" onClick={() => setError("")}>{error}</div>}
      {depts.map((d) => (
        <div key={d.id} className="room">
          <div className="room-head">
            <b>{d.icon} {d.name}</b>
            <button className="room-ws" title="Doi thu muc du an" onClick={() => pickWorkspace(d)}>{d.workspace || "Chon thu muc du an"}</button>
          </div>
          <div className="room-mission">{d.mission}</div>
          {d.errors.map((e) => <div key={e} className="office-error">{e}</div>)}
          {d.agent_defs.map((a) => (
            <div key={a.id} className="agent-row" title={a.description}>
              <span className="agent-name">{a.name}</span>
              <span className="agent-meta">{a.model ?? "default"} · {a.tools.join(", ")}</span>
              <button onClick={() => launch(d, a)}>Mo phien</button>
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}
