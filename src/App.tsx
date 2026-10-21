import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { load } from "@tauri-apps/plugin-store";
import Pane from "./Pane";
import UsageFooter from "./UsageFooter";
import Settings, { OfficePrefs, OFFICE_DEFAULTS } from "./Settings";
import Office, { AgentLaunch, Dept, Task } from "./Office";
import Drawer from "./Drawer";

/** task: id task Office gan voi tab (pane chat co dinh `${id}-chat-0`). */
type Ws = { id: string; cwd: string; agent?: AgentLaunch; task?: string };
type Started = { task: Task; label: string; args: string; settings: unknown };
type Stats = { context: number; cache_read: number; max_tokens: number };
type Chat = { n: number; args?: string };
type Entry = { id: string; title: string; modified: number };
type Prefs = { model: string; auto: boolean };

const MODELS = ["default", "opus", "sonnet", "haiku"];
const base = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;
const uid = () => Math.random().toString(36).slice(2, 10);

function Workspace({ ws, on, active, onFocus, onIdle, onEnter, onRatio, prefs, onPrefs }: { ws: Ws; on: boolean; active: boolean; onFocus: () => void; onIdle: () => void; onEnter: () => void; onRatio: (r: number) => void; prefs: Prefs; onPrefs: (p: Partial<Prefs>) => void }) {
  const [chat, setChat] = useState<Chat>({ n: 0 });
  const [model, setModel] = useState(prefs.model);
  const [auto, setAuto] = useState(prefs.auto);
  // Tab Agent: chi dung tham so cua Agent (model theo frontmatter, khong bao gio skip permissions).
  const flags = ws.agent
    ? `${ws.agent.args} ${chat.args ?? ""}`
    : `${model === "default" ? "" : `--model ${model}`}${auto ? " --dangerously-skip-permissions" : ""} ${chat.args ?? ""}`;
  const [hist, setHist] = useState<Entry[] | null>(null);
  const chatId = `${ws.id}-chat-${chat.n}`;

  const toggleHistory = async () =>
    setHist(hist ? null : await invoke<Entry[]>("list_sessions", { cwd: ws.cwd }));
  const restart = (args?: string) => {
    setChat({ n: chat.n + 1, args });
    setHist(null);
  };
  // Doi quyen phai khoi dong lai claude; --continue giu hoi thoai neu cwd da co session.
  const toggleAuto = async () => {
    setAuto(!auto);
    onPrefs({ auto: !auto });
    const has = (await invoke<Entry[]>("list_sessions", { cwd: ws.cwd })).length > 0;
    restart(has ? "--continue" : undefined);
  };

  // Keo thanh chia chat/shell: ti le dung chung cho moi tab (CSS var --r tren .stage).
  const drag = (e: React.PointerEvent<HTMLDivElement>) => {
    const bar = e.currentTarget;
    const top = bar.previousElementSibling!.getBoundingClientRect().top;
    const h = bar.nextElementSibling!.getBoundingClientRect().bottom - top;
    bar.setPointerCapture(e.pointerId);
    bar.onpointermove = (m) => onRatio(Math.min(0.9, Math.max(0.15, (m.clientY - top) / h)));
    bar.onpointerup = () => (bar.onpointermove = null);
  };

  return (
    <div className={`ws${on ? " on" : ""}${active ? " active" : ""}`} onMouseDown={onFocus}>
      <div className="head">
        <span className="head-title" title={ws.cwd} onDoubleClick={() => invoke("open_in_editor", { path: ws.cwd })}>{ws.agent ? `${ws.agent.label} · ${ws.cwd}` : ws.cwd}</span>
        {!ws.agent && <>
        <select
          className="model"
          value={model}
          onChange={(e) => {
            setModel(e.target.value);
            onPrefs({ model: e.target.value });
            invoke("pty_write", { id: chatId, data: `/model ${e.target.value}\r` });
          }}
        >
          {MODELS.map((m) => <option key={m}>{m}</option>)}
        </select>
        <label className="auto" title="Tu dong duyet moi lenh (--dangerously-skip-permissions)">
          <input type="checkbox" checked={auto} onChange={toggleAuto} /> auto
        </label>
        </>}
        {/* Tab task: phien gan voi task, khong cho doi sang chat khac. */}
        {!ws.task && <>
        <button className="icon" title="Chat moi" onClick={() => restart()}>＋</button>
        <button className="icon" title="Lich su" onClick={toggleHistory}>⟲</button>
        </>}
      </div>
      <div className="chat">
        <Pane key={chatId} id={chatId} cwd={ws.cwd} kind="chat" visible={on} args={flags} settings={ws.agent?.settings} onIdle={onIdle} onEnter={onEnter} />
      </div>
      <div className="splitter" onPointerDown={drag} />
      <div className="shell">
        <Pane id={`${ws.id}-shell`} cwd={ws.cwd} kind="shell" visible={on} />
      </div>
      {hist && (
        <div className="history">
          <div className="history-head">
            Lich su <button onClick={() => setHist(null)}>✕</button>
          </div>
          <div className="history-list">
            {hist.length === 0 && <div className="hint">Chua co session nao.</div>}
            {hist.map((h) => (
              <button key={h.id} className="history-item" onClick={() => restart(`--resume ${h.id}`)}>
                <span className="t">{h.title}</span>
                <span className="d">{new Date(h.modified).toLocaleString()}</span>
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

export default function App() {
  const [tabs, setTabs] = useState<Ws[]>([]);
  const [active, setActive] = useState<string | null>(null);
  const [ready, setReady] = useState(false);
  const [split, setSplit] = useState(false);
  const [ratio, setRatio] = useState(0.6);
  const [prefs, setPrefs] = useState<Prefs>({ model: "sonnet", auto: false });
  const [alerts, setAlerts] = useState<Set<string>>(new Set());
  const [office, setOffice] = useState<OfficePrefs>(OFFICE_DEFAULTS);
  const [settings, setSettings] = useState(false);
  const [view, setView] = useState<"office" | "terminal">("office");
  const [tasks, setTasks] = useState<Task[]>([]);
  const [names, setNames] = useState<Record<string, string>>({});
  const [drawer, setDrawer] = useState(false);
  const [ctx, setCtx] = useState<Record<string, number>>({});
  const [taskErr, setTaskErr] = useState("");
  const activeRef = useRef(active);
  activeRef.current = active;
  // Ref cho cac callback async (tin hieu hook, interval) luon thay state moi nhat.
  const tabsRef = useRef(tabs);
  tabsRef.current = tabs;
  const tasksRef = useRef(tasks);
  tasksRef.current = tasks;
  const officeRef = useRef(office);
  officeRef.current = office;

  const activate = (id: string) => {
    setActive(id);
    setView("terminal");
    setAlerts((a) => (a.has(id) ? new Set([...a].filter((x) => x !== id)) : a));
  };
  const alert = (id: string) => {
    if (id !== activeRef.current) setAlerts((a) => new Set(a).add(id));
  };

  useEffect(() => {
    load("state.json").then(async (s) => {
      const saved = (await s.get<string[]>("folders")) ?? [];
      const ws = saved.map((cwd) => ({ id: uid(), cwd }));
      setTabs(ws);
      setActive(ws[0]?.id ?? null);
      setSplit((await s.get<boolean>("split")) ?? false);
      setRatio((await s.get<number>("ratio")) ?? 0.6);
      setPrefs({ model: (await s.get<string>("model")) ?? "sonnet", auto: (await s.get<boolean>("auto")) ?? false });
      setOffice({
        root: (await s.get<string>("officeRoot")) || (await invoke<string>("office_root")),
        notes: (await s.get<string>("notesRoot")) ?? "",
        maxLive: (await s.get<number>("maxLive")) ?? OFFICE_DEFAULTS.maxLive,
        warnK: (await s.get<number>("warnK")) ?? OFFICE_DEFAULTS.warnK,
      });
      setReady(true);
    });
  }, []);

  useEffect(() => {
    if (!ready) return;
    load("state.json").then((s) => (s.set("folders", tabs.filter((t) => !t.agent).map((t) => t.cwd)), s.set("split", split), s.set("ratio", ratio), s.set("model", prefs.model), s.set("auto", prefs.auto), s.set("officeRoot", office.root), s.set("notesRoot", office.notes), s.set("maxLive", office.maxLive), s.set("warnK", office.warnK)));
  }, [tabs, split, ratio, prefs, office, ready]);

  const root = () => officeRef.current.root;
  const notesRoot = () => officeRef.current.notes || null;
  const fail = (e: unknown) => setTaskErr(String(e));
  const refresh = () => invoke<Task[]>("task_list", { root: root() }).then(setTasks, fail);

  const addTab = (ws: Ws, focus: boolean) => {
    tabsRef.current = [...tabsRef.current, ws];
    setTabs(tabsRef.current);
    if (focus) {
      setActive(ws.id);
      setView("terminal");
    } else setActive((a) => a ?? ws.id);
  };
  const tabFor = (s: Started): Ws => ({ id: uid(), cwd: s.task.cwd, agent: { label: s.label, args: s.args, settings: s.settings }, task: s.task.id });

  const close = (id: string) => {
    const tab = tabsRef.current.find((t) => t.id === id);
    const rest = tabsRef.current.filter((t) => t.id !== id);
    tabsRef.current = rest;
    setTabs(rest);
    if (activeRef.current === id) setActive(rest[rest.length - 1]?.id ?? null);
    return tab;
  };
  const closeTaskTab = (taskId: string) => tabsRef.current.filter((t) => t.task === taskId).forEach((t) => close(t.id));

  // Chay task den luot, mo tab nen (khong cuop man hinh). Rust giu khoa nen goi trung khong mo trung.
  const startNext = async () => {
    try {
      const started = await invoke<Started[]>("task_start_next", { root: root(), maxLive: officeRef.current.maxLive, notesRoot: notesRoot() });
      started.forEach((s) => addTab(tabFor(s), false));
    } catch (e) {
      fail(e);
    }
    await refresh();
  };

  // Lan chay truoc de lai task song nhung phien da mat: giu cho o waiting, mo lai qua ☰.
  useEffect(() => {
    if (!ready) return;
    (async () => {
      const ts = await invoke<Task[]>("task_list", { root: root() }).catch((e) => (fail(e), [] as Task[]));
      const orphan = ts.filter((t) => (t.status === "running" || t.status === "waiting") && !tabsRef.current.some((w) => w.task === t.id));
      for (const t of orphan) await invoke("task_stop", { root: root(), id: t.id, reason: "app dong" }).catch(fail);
      await startNext();
    })();
  }, [ready, office.root]);

  const onIdle = (ws: Ws) => {
    alert(ws.id);
    if (!ws.task) return;
    const was = tasksRef.current.find((t) => t.id === ws.task)?.status;
    invoke<Task>("task_idle", { root: root(), id: ws.task }).then((t) => {
      // Vua chuyen sang Cho sip: dong PTY giai phong cho (mo lai bang --resume). Tab mo lai tu ☰ thi giu.
      if (t.status === "review" && was !== "review") closeTaskTab(t.id);
      return startNext();
    }, fail);
  };
  const onEnter = (ws: Ws) => {
    const t = tasksRef.current.find((x) => x.id === ws.task);
    if (t?.status === "waiting") invoke("task_busy", { root: root(), id: t.id }).then(refresh, fail);
  };

  const approve = (t: Task) => {
    closeTaskTab(t.id);
    invoke("task_approve", { root: root(), id: t.id }).then(refresh, fail);
  };
  const rework = (t: Task, note: string) => {
    closeTaskTab(t.id);
    invoke("task_rework", { root: root(), id: t.id, note }).then(startNext, fail);
  };
  const openTask = async (t: Task) => {
    const tab = tabsRef.current.find((x) => x.task === t.id);
    if (tab) return activate(tab.id), setDrawer(false);
    try {
      addTab(tabFor(await invoke<Started>("task_resume", { root: root(), id: t.id, notesRoot: notesRoot() })), true);
      setDrawer(false);
    } catch (e) {
      fail(e);
    }
  };

  // Moi 20s: do dai ngu canh (canh bao) va tong token doc cache (vuot tran -> Ctrl+C).
  useEffect(() => {
    const iv = setInterval(async () => {
      for (const ws of tabsRef.current.filter((w) => w.task)) {
        const st = await invoke<Stats>("task_stats", { root: root(), id: ws.task }).catch(() => null);
        if (!st) continue;
        setCtx((c) => ({ ...c, [ws.task!]: st.context }));
        const t = tasksRef.current.find((x) => x.id === ws.task);
        if (t?.status === "running" && st.cache_read > st.max_tokens) {
          invoke("pty_write", { id: `${ws.id}-chat-0`, data: "\x03" });
          await invoke("task_stop", { root: root(), id: t.id, reason: "vuot tran" }).catch(fail);
          refresh();
        }
      }
    }, 20000);
    return () => clearInterval(iv);
  }, []);

  const openFolder = async () => {
    const cwd = await open({ directory: true });
    if (typeof cwd !== "string") return;
    addTab({ id: uid(), cwd }, true);
  };

  const openAgent = (cwd: string, agent: AgentLaunch) => addTab({ id: uid(), cwd, agent }, true);

  // Dong tab tay khi task con song -> task cho, giu cho de mo lai qua ☰.
  const closeByUser = (id: string) => {
    const tab = close(id);
    if (tab?.task) invoke("task_stop", { root: root(), id: tab.task, reason: "da dong tab" }).then(refresh, fail);
  };

  const pending = tasks.filter((t) => t.status === "review" || t.status === "waiting").length;

  return (
    <div className="app">
      <div className={`tabs${split ? " split" : ""}`}>
        <div className="views">
          <button className={`menu${drawer ? " on" : ""}`} title="Cho duyet" onClick={() => setDrawer(!drawer)}>
            ☰{pending > 0 && <span className="badge">{pending}</span>}
          </button>
          <button className={view === "office" ? "on" : ""} onClick={() => setView("office")}>Office</button>
          <button className={view === "terminal" ? "on" : ""} onClick={() => setView("terminal")}>Terminal</button>
        </div>
        {tabs.map((t) => (
          <div key={t.id} className={`tab${t.id === active ? " on" : ""}${alerts.has(t.id) ? " alert" : ""}`} onClick={() => activate(t.id)} title={t.cwd}>
            {t.agent?.label ?? base(t.cwd)}
            <button className="x" onClick={(e) => (e.stopPropagation(), closeByUser(t.id))}>×</button>
          </div>
        ))}
        <button className="add" onClick={openFolder}>+ Open folder</button>
        <div className="layout">
          <button className={split ? "" : "on"} title="Tung tab" onClick={() => setSplit(false)}>▭</button>
          <button className={split ? "on" : ""} title="Chia man hinh" onClick={() => setSplit(true)}>◫</button>
          <button className={settings ? "on" : ""} title="Settings" onClick={() => setSettings(!settings)}>⚙</button>
        </div>
      </div>
      {taskErr && <div className="office-error task-err" onClick={() => setTaskErr("")}>{taskErr}</div>}
      {view === "office" && (
        <Office
          prefs={office}
          tasks={tasks}
          ctx={ctx}
          onOpen={openAgent}
          onTasks={startNext}
          onDepts={(ds: Dept[]) => setNames(Object.fromEntries(ds.flatMap((d) => d.agent_defs.map((a) => [`${d.id}/${a.id}`, `${a.name} · ${d.name}`]))))}
        />
      )}
      <div className={`stage${split ? " split" : ""}${view === "office" ? " hidden" : ""}`} style={{ "--r": ratio } as React.CSSProperties}>
        {tabs.length === 0 && (
          <div className="empty">
            <button onClick={openFolder}>Mo thu muc de bat dau</button>
          </div>
        )}
        {tabs.map((t) => (
          <Workspace
            key={t.id}
            ws={t}
            on={view === "terminal" && (split || t.id === active)}
            active={split && t.id === active}
            onFocus={() => activate(t.id)}
            onIdle={() => onIdle(t)}
            onEnter={() => onEnter(t)}
            onRatio={setRatio}
            prefs={prefs}
            onPrefs={(p) => setPrefs((x) => ({ ...x, ...p }))}
          />
        ))}
      </div>
      {drawer && <Drawer tasks={tasks} names={names} onApprove={approve} onRework={rework} onOpen={openTask} onClose={() => setDrawer(false)} />}
      {settings && <Settings value={office} onSave={setOffice} onClose={() => setSettings(false)} />}
      <UsageFooter />
    </div>
  );
}
