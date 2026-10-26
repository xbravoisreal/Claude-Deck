import { Sprite, useTheme, characters, cellOf } from "./theme";
import { bubble, type Dept, type Task, type AgentDef } from "./Office";

type Seat = { x: number; y: number; character?: string };
type Pos = { x: number; y: number };
/** sign: vi tri bien hieu (ten + mo ta phong); workspace: vi tri ke sach chua thu muc du an, w = so o chieu ngang (mac dinh 3). */
type RoomCfg = { cols?: number; rows?: number; floor?: string; decor?: ({ slot: string } & Pos)[]; seats?: Record<string, Seat>; sign?: Pos; workspace?: Pos & { w?: number } };

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
    // Dai b gom 2 hang (2+3b, 3+3b); hang 1 de trong cho bong thoai (khong de len bien hieu tren tuong);
    // cot 1,3,5... luan phien hang tren/duoi; giua cac dai chua 1 hang trong cho nhan.
    for (let b = 0; !seats[a]; b++) {
      for (let k = 0; 1 + 2 * k < cols - 1 && !seats[a]; k++) {
        const x = 1 + 2 * k, y = 2 + 3 * b + (k % 2);
        if (free(x, y)) {
          seats[a] = { x, y };
          used.add(`${x},${y}`);
        }
      }
      if (b > agents.length) seats[a] = { x: 1, y: 2 + 3 * b }; // phong qua hep: xep doc
    }
  }
  const all = Object.values(seats);
  return {
    seats,
    cols: Math.max(cols, ...all.map((s) => s.x + 1)),
    rows: Math.max(rows, ...all.map((s) => s.y + 1)),
  };
}

const HIRE = "__hire";

/** Trang thai nhan vat theo task hien tai. */
const pose = (t?: Task) => (t?.status === "running" ? "typing" : t?.status === "waiting" || t?.status === "review" ? "waiting" : "idle");

const ago = (ms: number) => {
  const m = Math.round((Date.now() - ms) / 60000);
  return m < 1 ? "vua xong" : m < 60 ? `${m} phut` : `${Math.round(m / 60)} gio`;
};

export default function Room({ dept, tasks, ctx, tools, warnK, onDesk, onHistory, onWorkspace, onHire, onSignMenu }: {
  dept: Dept; tasks: Task[]; ctx: Record<string, number>; tools: Record<string, string>; warnK: number;
  onDesk: (a: AgentDef) => void; onHistory: () => void; onWorkspace: () => void; onHire: () => void; onSignMenu: (x: number, y: number) => void;
}) {
  const theme = useTheme()!;
  const cell = cellOf(theme);
  const room = (dept.room ?? {}) as RoomCfg;
  const shelfW = Math.max(1, room.workspace?.w ?? 3);
  // Agent da nghi khong co ban; ban cuoi "__hire" = vi tri chua tuyen (+ Tuyen).
  const status = (id: string) => dept.status?.[id] ?? "active";
  const staff = dept.agent_defs.filter((a) => status(a.id) !== "retired");
  const { seats, cols: seatCols, rows } = seatsFor(room, [...staff.map((a) => a.id), HIRE], room.cols ?? Math.max(6, staff.length * 2 + 4), room.rows ?? 3);
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

  const sign = room.sign ?? { x: 0, y: 0 };
  const shelf = room.workspace ?? { x: Math.max(0, seatCols - shelfW), y: 0 };
  const cols = Math.max(seatCols, shelf.x + shelfW);
  const slice = theme.sprite.signSlice ?? 8;
  const shelfSlice = theme.sprite.shelfSlice ?? 8;
  const wsName = dept.workspace.split(/[\/]/).filter(Boolean).pop();

  // Ca the phong la canh: san/tuong lat kin, bien hieu + ke sach nam tren tuong, "Da xong" noi o goc duoi.
  return (
    <div className="room-scroll">
      <div
        className="room-floor"
        style={{
          minWidth: cols * cell,
          minHeight: rows * cell + 28, // + cho nhan duoi ban hang cuoi
          backgroundImage: [theme.slots.wall, theme.slots[floor]].filter(Boolean).map((u) => `url(${u})`).join(","),
          backgroundRepeat: "repeat-x, repeat",
          backgroundSize: `${cell}px ${cell}px`,
        }}
      >
        {(room.decor ?? []).map((d, i) => (
          <Sprite key={i} slot={d.slot} style={{ left: d.x * cell, top: d.y * cell }} />
        ))}
        <div
          className={`room-sign${theme.slots.sign ? "" : " plain"}`}
          title="Bam phai de sua / dong phong"
          onContextMenu={(e) => (e.preventDefault(), onSignMenu(e.clientX, e.clientY))}
          style={{
            left: sign.x * cell + 6,
            top: sign.y * cell + 4,
            // Bien hieu khong dai qua 4 o va khong de len ke thu muc cung hang.
            maxWidth: Math.max(2, Math.min(4, shelf.y === sign.y && shelf.x > sign.x ? shelf.x - sign.x : 4)) * cell - 12,
            borderWidth: px(slice / 2),
            borderImage: theme.slots.sign ? `url(${theme.slots.sign}) ${slice} fill / ${px(slice / 2)}px stretch` : undefined,
          }}
        >
          <b>{dept.icon} {dept.name}</b>
          <span className="room-mission">{dept.mission}</span>
          <span className="room-pills">
            {count("review") > 0 && <span className="pill review">{count("review")} Chờ síp duyệt</span>}
            {count("waiting") > 0 && <span className="pill waiting">{count("waiting")} Chờ síp trả lời</span>}
            {count("running") > 0 && <span className="pill running">{count("running")} Dang lam</span>}
            {count("backlog") > 0 && <span className="pill">{count("backlog")} hang doi</span>}
          </span>
        </div>
        {/* Ke thu muc du an: anh 9 o keo gian ngang theo w o. */}
        <button
          className="room-shelf"
          style={{
            left: shelf.x * cell,
            top: shelf.y * cell,
            width: shelfW * cell,
            borderWidth: px(shelfSlice / 2),
            borderImage: theme.slots.workspace ? `url(${theme.slots.workspace}) ${shelfSlice} fill / ${px(shelfSlice / 2)}px stretch` : undefined,
          }}
          title={dept.workspace || "Chon thu muc du an"}
          onClick={onWorkspace}
        >
          {/* rtl + ltr ben trong: path qua dai thi cat dau, giu ten thu muc cuoi */}
          <span className="shelf-label"><span dir="ltr">{dept.workspace || "Chon thu muc du an"}</span></span>
        </button>
        <button className="seat hire" style={{ left: seats[HIRE].x * cell, top: seats[HIRE].y * cell }} title="Tuyển Agent mới cho phòng (qua HR)" onClick={onHire}>
          + Tuyển
        </button>
        {staff.map((a) => {
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
                {t && <span className={`bubble ${t.status}`} style={{ bottom: `calc(100% + ${6 - Math.min(0, px(theme.sprite.characterY))}px)` }}>{t.status === "waiting" ? "Chờ síp trả lời" : bubble(t, tools[t.id])}</span>}
                <span className="seat-label">
                  <b>{a.name}</b>
                  {status(a.id) === "probation" && <span className="probation">Thử việc</span>}
                  {t && <span>{t.title} · {ago(t.created)}</span>}
                </span>
              </button>
            );
          })}
        {done.length > 0 && (
          <div className="room-done">
            <span>Da xong {done.length}</span>
            {done.slice(0, 5).map((t) => (
              <button key={t.id} className="chip" title={`${t.id} · ${t.summary}`} onClick={onHistory}>{t.title}</button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
