import { Sprite, useTheme, characters, cellOf } from "./theme";
import { bubble, type Dept, type Task, type AgentDef } from "./Office";

type Seat = { x: number; y: number; character?: string };
type RoomCfg = { cols?: number; rows?: number; floor?: string; decor?: { slot: string; x: number; y: number }[]; seats?: Record<string, Seat> };

const hash = (s: string) => [...s].reduce((h, c) => (h * 31 + c.charCodeAt(0)) >>> 0, 7);

/** Cho ngoi: theo room.json, agent thieu cho thi xep vao o trong ke tiep (hang 1 tro xuong, cach 1 o). */
export function seatsFor(room: RoomCfg, agents: string[], cols: number, rows: number) {
  const out: Record<string, Seat> = {};
  const taken = new Set((room.decor ?? []).map((d) => `${d.x},${d.y}`));
  for (const a of agents) {
    const s = room.seats?.[a];
    if (s) {
      out[a] = s;
      taken.add(`${s.x},${s.y}`);
    }
  }
  let i = 0;
  for (const a of agents.filter((a) => !out[a])) {
    for (; i < cols * rows; i++) {
      const x = 1 + ((i * 2) % Math.max(1, cols - 1)), y = 1 + Math.floor((i * 2) / Math.max(1, cols - 1));
      if (y < rows && !taken.has(`${x},${y}`)) {
        out[a] = { x, y };
        taken.add(`${x},${y}`);
        break;
      }
    }
    out[a] ??= { x: 0, y: rows - 1 }; // phong qua chat: ngoi tam goc duoi
  }
  return out;
}

/** Trang thai nhan vat theo task hien tai. */
const pose = (t?: Task) => (t?.status === "running" ? "typing" : t?.status === "waiting" || t?.status === "review" ? "waiting" : "idle");

const ago = (ms: number) => {
  const m = Math.round((Date.now() - ms) / 60000);
  return m < 1 ? "vua xong" : m < 60 ? `${m} phut` : `${Math.round(m / 60)} gio`;
};

export default function Room({ dept, tasks, ctx, tools, warnK, onDesk, onHistory }: {
  dept: Dept; tasks: Task[]; ctx: Record<string, number>; tools: Record<string, string>; warnK: number;
  onDesk: (a: AgentDef) => void; onHistory: () => void;
}) {
  const theme = useTheme()!;
  const cell = cellOf(theme);
  const room = (dept.room ?? {}) as RoomCfg;
  const cols = room.cols ?? Math.max(6, dept.agent_defs.length * 2 + 2);
  const rows = room.rows ?? 3;
  const floor = room.floor && theme.slots[room.floor] ? room.floor : Object.keys(theme.slots).find((k) => k.startsWith("floor.")) ?? "";
  const seats = seatsFor(room, dept.agent_defs.map((a) => a.id), cols, rows);
  const chars = characters(theme);
  const mine = tasks.filter((t) => t.dept === dept.id);
  const current = (a: string) =>
    mine.find((t) => t.agent === a && (t.status === "running" || t.status === "waiting")) ?? mine.find((t) => t.agent === a && t.status === "review");
  const count = (s: Task["status"]) => mine.filter((t) => t.status === s).length;
  const done = mine.filter((t) => t.status === "done" && t.reason !== "da huy").sort((a, b) => (b.finished || b.created) - (a.finished || a.created));
  const px = (v = 0) => v * theme.sprite.scale;

  return (
    <>
      <div className="room-pills">
        {count("review") > 0 && <span className="pill review">{count("review")} Chờ síp</span>}
        {count("waiting") > 0 && <span className="pill waiting">{count("waiting")} Chờ anh</span>}
        {count("running") > 0 && <span className="pill running">{count("running")} Dang lam</span>}
        {count("backlog") > 0 && <span className="pill">{count("backlog")} hang doi</span>}
      </div>
      <div className="room-scroll">
        <div
          className="room-floor"
          style={{
            width: cols * cell,
            height: rows * cell,
            backgroundImage: [theme.slots.wall, theme.slots[floor]].filter(Boolean).map((u) => `url(${u})`).join(","),
            backgroundRepeat: "repeat-x, repeat",
            backgroundSize: `${cell}px ${cell}px`,
          }}
        >
          {(room.decor ?? []).map((d, i) => (
            <Sprite key={i} slot={d.slot} style={{ left: d.x * cell, top: d.y * cell }} />
          ))}
          {dept.agent_defs.map((a) => {
            const s = seats[a.id];
            const t = current(a.id);
            const ch = s.character?.replace(/^character\./, "") ?? chars[hash(a.id) % Math.max(1, chars.length)];
            const warn = t && (ctx[t.id] ?? 0) >= warnK * 1000;
            return (
              <button
                key={a.id}
                className={`seat${warn ? " warn" : ""}`}
                style={{ left: s.x * cell, top: s.y * cell }}
                title={warn ? "phien dai, nen tach task" : `${a.name}: ${a.description}`}
                onClick={() => onDesk(a)}
              >
                <Sprite slot={`character.${ch}.${pose(t)}`} style={{ top: px(theme.sprite.characterY) }} />
                <Sprite slot="desk" style={{ top: px(theme.sprite.deskY) }} />
                {t && <span className={`bubble ${t.status}`} style={{ bottom: `calc(100% + ${6 - Math.min(0, px(theme.sprite.characterY))}px)` }}>{t.status === "waiting" ? "Chờ anh" : bubble(t, tools[t.id])}</span>}
                <span className="seat-label">
                  <b>{a.name}</b>
                  {t && <span>{t.title} · {ago(t.created)}</span>}
                </span>
              </button>
            );
          })}
        </div>
      </div>
      {done.length > 0 && (
        <div className="room-done">
          <span>Da xong {done.length}</span>
          {done.slice(0, 5).map((t) => (
            <button key={t.id} className="chip" title={`${t.id} · ${t.summary}`} onClick={onHistory}>{t.title}</button>
          ))}
        </div>
      )}
    </>
  );
}
