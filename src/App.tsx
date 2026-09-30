import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { load } from "@tauri-apps/plugin-store";
import Pane from "./Pane";
import UsageFooter from "./UsageFooter";

type Ws = { id: string; cwd: string };
type Chat = { n: number; args?: string };
type Entry = { id: string; title: string; modified: number };

const MODELS = ["default", "opus", "sonnet", "haiku"];
const base = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;
const uid = () => Math.random().toString(36).slice(2, 10);

function Workspace({ ws, on, active, onFocus, onIdle }: { ws: Ws; on: boolean; active: boolean; onFocus: () => void; onIdle: () => void }) {
  const [chat, setChat] = useState<Chat>({ n: 0 });
  const [hist, setHist] = useState<Entry[] | null>(null);
  const chatId = `${ws.id}-chat-${chat.n}`;

  const toggleHistory = async () =>
    setHist(hist ? null : await invoke<Entry[]>("list_sessions", { cwd: ws.cwd }));
  const restart = (args?: string) => {
    setChat({ n: chat.n + 1, args });
    setHist(null);
  };

  return (
    <div className={`ws${on ? " on" : ""}${active ? " active" : ""}`} onMouseDown={onFocus}>
      <div className="head">
        <span className="head-title" title={ws.cwd}>{ws.cwd}</span>
        <select
          className="model"
          defaultValue="default"
          onChange={(e) => invoke("pty_write", { id: chatId, data: `/model ${e.target.value}\r` })}
        >
          {MODELS.map((m) => <option key={m}>{m}</option>)}
        </select>
        <button className="icon" title="Chat moi" onClick={() => restart()}>＋</button>
        <button className="icon" title="Lich su" onClick={toggleHistory}>⟲</button>
      </div>
      <div className="chat">
        <Pane key={chatId} id={chatId} cwd={ws.cwd} kind="chat" visible={on} args={chat.args} onIdle={onIdle} />
      </div>
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
      setReady(true);
    });
  }, []);

  useEffect(() => {
    if (!ready) return;
    load("state.json").then((s) => (s.set("folders", tabs.map((t) => t.cwd)), s.set("split", split)));
  }, [tabs, split, ready]);

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
      <div className="tabs">
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
      <div className={`stage${split ? " split" : ""}`}>
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
          />
        ))}
      </div>
      <UsageFooter />
    </div>
  );
}
