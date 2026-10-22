import { Sprite, useTheme, characters, cellOf } from "./theme";
import { bubble, type Dept, type Task, type AgentDef } from "./Office";

type Seat = { x: number; y: number; character?: string };
type RoomCfg = { cols?: number; rows?: number; floor?: string; decor?: { slot: string; x: number; y: number }[]; seats?: Record<string, Seat> };

const hash = (s: string) => [...s].reduce((h, c) => (h * 31 + c.charCodeAt(0)) >>> 0, 7);

/** Cho ngoi: theo room.json; agent thieu cho -> xep zic zac (cot cach 2 o, hang so le lech xuong 1) de nhan duoi ban
 *  khong de len bong thoai/ban ben canh; bo o dung decor hoac sat ban khac. Het cho thi noi them hang.
 *  Tra ve ca kich thuoc phong da noi rong cho vua moi ban. */
export function seatsFor(room: RoomCfg, agents: string[], cols: number, rows: number) {
  const seats: Record<string, Seat> = {};
  const decor = new Set((room.decor ?? []).map((d) => `${d.x},${d.y}`));
  const used = new Set<string>();
  for (const a of agents) {
    const s = room.seats?.[a];
    if (s) {
      seats[a] = s;
      used.add(`${s.x},${s.y}`);
    }
  }
  const free = (x: number, y: number) => !decor.has(`${x},${y}`) && ![x - 1, x, x + 1].some((k) => used.has(`${k},${y}`));
  for (const a of agents.filter((a) => !seats[a])) {
    // Dai b gom 2 hang (1+3b, 2+3b); cot 1,3,5... luan phien hang tren/duoi; giua cac dai chua 1 hang trong cho nhan.
    for (let b = 0; !seats[a]; b++) {
      for (let k = 0; 1 + 2 * k < cols - 1 && !seats[a]; k++) {
        const x = 1 + 2 * k, y = 1 + 3 * b + (k % 2);
        if (free(x, y)) {
          seats[a] = { x, y };
          used.add(`${x},${y}`);
        }
      }
      if (b > agents.length) seats[a] = { x: 1, y: 1 + 3 * b }; // phong qua hep: xep doc
    }
  }
  const all = Object.values(seats);
  return {
    seats,
    cols: Math.max(cols, ...all.map((s) => s.x + 1)),
    rows: Math.max(rows, ...all.map((s) => s.y + 1)),
  };
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
  const { seats, cols, rows } = seatsFor(room, dept.agent_defs.map((a) => a.id), room.cols ?? Math.max(6, dept.agent_defs.length * 2 + 2), room.rows ?? 3);
  const floor = room.floor && theme.slots[room.floor] ? room.floor : Object.keys(theme.slots).find((k) => k.startsWith("floor.")) ?? "";
  const chars = characters(theme);
  /** Slot nhan vat cho tu the; thieu anh tu the thi lui ve typing, roi ve nhan vat mac dinh theo ten. */
  const charSlot = (want: string | undefined, agent: string, p: string) => {
    const fallback = chars[hash(agent) % Math.max(1, chars.length)];
    for (const id of [want, fallback]) for (const s of [p, "typing"]) if (id && theme.slots[`character.${id}.${s}`]) return `character.${id}.${s}`;
    return "";
  };
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
            const warn = t && (ctx[t.id] ?? 0) >= warnK * 1000;
            return (
              <button
                key={a.id}
                className={`seat${warn ? " warn" : ""}`}
                style={{ left: s.x * cell, top: s.y * cell }}
                title={warn ? "phien dai, nen tach task" : `${a.name}: ${a.description}`}
                onClick={() => onDesk(a)}
              >
                <Sprite slot={charSlot(s.character?.replace(/^character\./, ""), a.id, pose(t))} style={{ top: px(theme.sprite.characterY) }} />
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
