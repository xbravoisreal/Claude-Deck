import { useRef, useState } from "react";
import { Sprite, useTheme, characters, cellOf } from "./theme";
import { bubble, type Dept, type Task, type AgentDef } from "./Office";

type Seat = { x: number; y: number; character?: string };
type Pos = { x: number; y: number };
/** Kich thuoc phong: 2 muc rong (1 | 2 cot luoi) x 2 muc cao (vua | cao them 3 o). */
export type Size = { w: 1 | 2; h: 1 | 2 };
/** sign: vi tri bien hieu (ten + mo ta phong); workspace: vi tri ke sach chua thu muc du an, w = so o chieu ngang (mac dinh 3). */
type RoomCfg = { cols?: number; rows?: number; floor?: string; decor?: ({ slot: string } & Pos)[]; seats?: Record<string, Seat>; sign?: Pos; workspace?: Pos & { w?: number }; size?: Size };

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

/** Trang thai nhan vat theo task hien tai. */
const pose = (t?: Task) => (t?.status === "running" ? "typing" : t?.status === "waiting" || t?.status === "review" ? "waiting" : "idle");

const ago = (ms: number) => {
  const m = Math.round((Date.now() - ms) / 60000);
  return m < 1 ? "vua xong" : m < 60 ? `${m} phut` : `${Math.round(m / 60)} gio`;
};

/** Kich thuoc phong: room.json `size`, thieu thi rong 2 khi co tren 3 Agent. */
export const roomSize = (room: RoomCfg, agents: number): Size => ({ w: room.size?.w === 1 || room.size?.w === 2 ? room.size.w : agents > 3 ? 2 : 1, h: room.size?.h === 2 ? 2 : 1 });

const DRAG_PX = 5; // keo it hon nguong nay van tinh la bam

export default function Room({ dept, tasks, ctx, tools, delegated, warnK, onDesk, onHistory, onWorkspace, onHire, onSignMenu, onSeats, onRoomDrop, onSize }: {
  dept: Dept; tasks: Task[]; ctx: Record<string, number>; tools: Record<string, string>; delegated: string[]; warnK: number;
  onDesk: (a: AgentDef) => void; onHistory: () => void; onWorkspace: () => void; onHire: () => void; onSignMenu: (x: number, y: number) => void;
  onSeats: (seats: Record<string, Seat>) => void; onRoomDrop: (x: number, y: number) => void; onSize: (size: Size) => void;
}) {
  // Keo ban: tha vao o luoi gan nhat (diem neo); o co Agent khac thi doi cho, o bi chan thi tra ve cho cu.
  const [drag, setDrag] = useState<{ id: string; dx: number; dy: number } | null>(null);
  const start = useRef<{ id: string; x: number; y: number } | null>(null);
  const moved = useRef(false);
  const grip = useRef<Pos | null>(null);
  const theme = useTheme()!;
  const cell = cellOf(theme);
  const room = (dept.room ?? {}) as RoomCfg;
  const shelfW = Math.max(1, room.workspace?.w ?? 3);
  // Agent da nghi khong co ban; ban cuoi "__hire" = vi tri chua tuyen (+ Tuyen).
  const status = (id: string) => dept.status?.[id] ?? "active";
  const staff = dept.agent_defs.filter((a) => status(a.id) !== "retired");
  const { seats, cols: seatCols, rows } = seatsFor(room, staff.map((a) => a.id), room.cols ?? Math.max(6, staff.length * 2 + 4), room.rows ?? 3);
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

  const size = roomSize(room, staff.length);
  const sign = room.sign ?? { x: 0, y: 0 };
  const shelf = room.workspace ?? { x: Math.max(0, seatCols - shelfW), y: 0 };
  const cols = Math.max(seatCols, shelf.x + shelfW);
  const slice = theme.sprite.signSlice ?? 8;
  const shelfSlice = theme.sprite.shelfSlice ?? 8;
  const wsName = dept.workspace.split(/[\/]/).filter(Boolean).pop();

  /** O dich cua ban dang keo (da kep trong phong), hoac null neu bi chan (bien hieu, ke sach, decor, ban "+ Tuyen"). */
  const snap = (id: string, dx: number, dy: number) => {
    const s = seats[id];
    const x = Math.min(cols - 1, Math.max(0, Math.round((s.x * cell + dx) / cell)));
    const y = Math.max(1, Math.round((s.y * cell + dy) / cell));
    const blocked =
      (room.decor ?? []).some((d) => d.x === x && d.y === y) ||
      (y === shelf.y && x >= shelf.x && x < shelf.x + shelfW) ||
      (y === sign.y && x >= sign.x && x < sign.x + 4);
    return blocked ? null : { x, y };
  };
  const dropSeat = () => {
    if (!drag) return;
    const to = snap(drag.id, drag.dx, drag.dy);
    setDrag(null);
    if (!to) return;
    const from = seats[drag.id];
    const other = staff.find((a) => a.id !== drag.id && seats[a.id].x === to.x && seats[a.id].y === to.y);
    if (!other && from.x === to.x && from.y === to.y) return;
    // Ghi het cho ngoi cua phong de bo cuc khong bi xep lai khi them/bot Agent.
    const next: Record<string, Seat> = Object.fromEntries(staff.map((a) => [a.id, seats[a.id]]));
    next[drag.id] = { ...from, ...to };
    if (other) next[other.id] = { ...seats[other.id], x: from.x, y: from.y };
    onSeats(next);
  };
  const ghost = drag && snap(drag.id, drag.dx, drag.dy);

  // Ca the phong la canh: san/tuong lat kin, bien hieu + ke sach nam tren tuong, "Da xong" noi o goc duoi.
  return (
    <div className="room-scroll">
      <div
        className="room-floor"
        style={{
          minWidth: cols * cell,
          minHeight: (rows + (size.h === 2 ? 3 : 0)) * cell + 28, // + cho nhan duoi ban hang cuoi
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
          title="Bam phai de sua / dong phong; keo sang phong khac de doi cho"
          onContextMenu={(e) => (e.preventDefault(), onSignMenu(e.clientX, e.clientY))}
          onPointerDown={(e) => {
            if (e.button !== 0) return;
            e.currentTarget.setPointerCapture(e.pointerId);
            start.current = { id: "", x: e.clientX, y: e.clientY };
          }}
          onPointerUp={(e) => {
            const s = start.current;
            start.current = null;
            if (s && Math.hypot(e.clientX - s.x, e.clientY - s.y) > DRAG_PX) onRoomDrop(e.clientX, e.clientY);
          }}
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
        <button className="hire-btn" title="Tuyển Agent mới cho phòng (qua HR)" onClick={onHire}>+ Tuyển</button>
        <div
          className="room-grip"
          title="Keo de doi kich thuoc phong (2 muc rong x 2 muc cao)"
          onPointerDown={(e) => (e.currentTarget.setPointerCapture(e.pointerId), (grip.current = { x: e.clientX, y: e.clientY }))}
          onPointerUp={(e) => {
            const g = grip.current;
            grip.current = null;
            if (!g) return;
            const dx = e.clientX - g.x, dy = e.clientY - g.y;
            const next: Size = { w: Math.abs(dx) > 40 ? (dx > 0 ? 2 : 1) : size.w, h: Math.abs(dy) > 40 ? (dy > 0 ? 2 : 1) : size.h };
            if (next.w !== size.w || next.h !== size.h) onSize(next);
          }}
        />
        {ghost && <div className="seat-ghost" style={{ left: ghost.x * cell, top: ghost.y * cell }} />}
        {staff.map((a) => {
            const s = seats[a.id];
            const dragging = drag?.id === a.id;
            const t = current(a.id);
            // Lead dang giao viec cho Agent nay (subagent) -> ngoi go phim du khong co task rieng.
            const sub = !t && delegated.includes(a.id);
            const warn = t && (ctx[t.id] ?? 0) >= warnK * 1000;
            return (
              <button
                key={a.id}
                className={`seat${warn ? " warn" : ""}${dragging ? " dragging" : ""}`}
                style={{ left: s.x * cell, top: s.y * cell, transform: dragging ? `translate(${drag.dx}px, ${drag.dy}px)` : undefined }}
                title={warn ? "phien dai, nen tach task" : `${a.name}: ${a.description}`}
                onPointerDown={(e) => {
                  if (e.button !== 0) return;
                  e.currentTarget.setPointerCapture(e.pointerId);
                  start.current = { id: a.id, x: e.clientX, y: e.clientY };
                  moved.current = false;
                }}
                onPointerMove={(e) => {
                  const st = start.current;
                  if (!st || st.id !== a.id) return;
                  const dx = e.clientX - st.x, dy = e.clientY - st.y;
                  if (moved.current || Math.hypot(dx, dy) > DRAG_PX) (moved.current = true, setDrag({ id: a.id, dx, dy }));
                }}
                onPointerUp={() => {
                  start.current = null;
                  dropSeat();
                }}
                onPointerCancel={() => (start.current = null, setDrag(null))}
                onClick={() => (moved.current ? (moved.current = false) : onDesk(a))}
              >
                <Sprite slot={charSlot(s.character?.replace(/^character\./, ""), a.id, sub ? "typing" : pose(t))} style={{ top: px(theme.sprite.characterY) }} />
                <Sprite slot="desk" style={{ top: px(theme.sprite.deskY) }} />
                {t && <span className={`bubble ${t.status}`} style={{ bottom: `calc(100% + ${6 - Math.min(0, px(theme.sprite.characterY))}px)` }}>{t.status === "waiting" ? "Chờ síp trả lời" : bubble(t, tools[t.id])}</span>}
                {sub && <span className="bubble running" style={{ bottom: `calc(100% + ${6 - Math.min(0, px(theme.sprite.characterY))}px)` }}>Đang làm (Lead giao)</span>}
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
