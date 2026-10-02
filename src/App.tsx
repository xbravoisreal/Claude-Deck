import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { load } from "@tauri-apps/plugin-store";
import Pane from "./Pane";
import UsageFooter from "./UsageFooter";

type Ws = { id: string; cwd: string };
type Chat = { n: number; args?: string };
type Entry = { id: string; title: string; modified: number };
type Prefs = { model: string; auto: boolean };

const MODELS = ["default", "opus", "sonnet", "haiku"];
const base = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;
const uid = () => Math.random().toString(36).slice(2, 10);

function Workspace({ ws, on, active, onFocus, onIdle, onRatio, prefs, onPrefs }: { ws: Ws; on: boolean; active: boolean; onFocus: () => void; onIdle: () => void; onRatio: (r: number) => void; prefs: Prefs; onPrefs: (p: Partial<Prefs>) => void }) {
  const [chat, setChat] = useState<Chat>({ n: 0 });
  const [model, setModel] = useState(prefs.model);
  const [auto, setAuto] = useState(prefs.auto);
  const flags = `${model === "default" ? "" : `--model ${model}`}${auto ? " --dangerously-skip-permissions" : ""} ${chat.args ?? ""}`;
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
        <span className="head-title" title={ws.cwd}>{ws.cwd}</span>
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
        <button className="icon" title="Chat moi" onClick={() => restart()}>＋</button>
        <button className="icon" title="Lich su" onClick={toggleHistory}>⟲</button>
      </div>
      <div className="chat">
        <Pane key={chatId} id={chatId} cwd={ws.cwd} kind="chat" visible={on} args={flags} onIdle={onIdle} />
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
  const activeRef = useRef(active);
  activeRef.current = active;

  const activate = (id: string) => {
    setActive(id);
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
      setReady(true);
    });
  }, []);

  useEffect(() => {
    if (!ready) return;
    load("state.json").then((s) => (s.set("folders", tabs.map((t) => t.cwd)), s.set("split", split), s.set("ratio", ratio), s.set("model", prefs.model), s.set("auto", prefs.auto)));
  }, [tabs, split, ratio, prefs, ready]);

  const openFolder = async () => {
    const cwd = await open({ directory: true });
    if (typeof cwd !== "string") return;
    const ws = { id: uid(), cwd };
    setTabs((t) => [...t, ws]);
    setActive(ws.id);
  };

  const close = (id: string) => {
    const rest = tabs.filter((t) => t.id !== id);
    setTabs(rest);
    if (active === id) setActive(rest[rest.length - 1]?.id ?? null);
  };

  return (
    <div className="app">
      <div className={`tabs${split ? " split" : ""}`}>
        {tabs.map((t) => (
          <div key={t.id} className={`tab${t.id === active ? " on" : ""}${alerts.has(t.id) ? " alert" : ""}`} onClick={() => activate(t.id)} title={t.cwd}>
            {base(t.cwd)}
            <button className="x" onClick={(e) => (e.stopPropagation(), close(t.id))}>×</button>
          </div>
        ))}
        <button className="add" onClick={openFolder}>+ Open folder</button>
        <div className="layout">
          <button className={split ? "" : "on"} title="Tung tab" onClick={() => setSplit(false)}>▭</button>
          <button className={split ? "on" : ""} title="Chia man hinh" onClick={() => setSplit(true)}>◫</button>
        </div>
      </div>
      <div className={`stage${split ? " split" : ""}`} style={{ "--r": ratio } as React.CSSProperties}>
        {tabs.length === 0 && (
          <div className="empty">
            <button onClick={openFolder}>Mo thu muc de bat dau</button>
          </div>
        )}
        {tabs.map((t) => (
          <Workspace
            key={t.id}
            ws={t}
            on={split || t.id === active}
            active={split && t.id === active}
            onFocus={() => activate(t.id)}
            onIdle={() => alert(t.id)}
            onRatio={setRatio}
            prefs={prefs}
            onPrefs={(p) => setPrefs((x) => ({ ...x, ...p }))}
          />
        ))}
      </div>
      <UsageFooter />
    </div>
  );
}
