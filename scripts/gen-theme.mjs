// Sinh theme mac dinh (pixel-art SVG) tu luoi ky tu 16x16. Chay: node scripts/gen-theme.mjs
// Moi o luoi = 1 pixel, '.' = trong suot. SVG ra 32x32 moi khung (phong 2 lan), dai khung xep ngang.
// Ghi: src-tauri/office-seed/themes/default/*, src-tauri/src/theme_files.rs, va trang xem thu trong thu muc tam.
import { writeFileSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const OUT = "src-tauri/office-seed/themes/default";
const FRAME = 32;

/** frames: mang luoi (moi luoi 16 dong x 16 ky tu); pal: ky tu -> mau. Gop cac pixel cung mau lien nhau tren dong. */
function svg(frames, pal) {
  const n = frames.length;
  let rects = "";
  frames.forEach((g, f) => {
    if (g.length !== 16 || g.some((r) => r.length !== 16)) throw new Error("luoi phai 16x16: " + JSON.stringify(g));
    g.forEach((row, y) => {
      for (let x = 0; x < 16; ) {
        const c = row[x];
        let w = 1;
        while (x + w < 16 && row[x + w] === c) w++;
        if (c !== ".") {
          if (!pal[c]) throw new Error("thieu mau cho '" + c + "'");
          rects += `<rect x="${f * 16 + x}" y="${y}" width="${w}" height="1" fill="${pal[c]}"/>`;
        }
        x += w;
      }
    });
  });
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${FRAME * n}" height="${FRAME}" viewBox="0 0 ${16 * n} 16" shape-rendering="crispEdges">${rects}</svg>\n`;
}

// ---- Nhan vat chibi ngoi ban (ban ve de len nua duoi) ----
const HEAD = [
  "................",
  ".....HHHHHH.....",
  "....HHHHHHHH....",
  "...HHHHHHHHHH...",
  "...HHSSSSSSHH...",
  "...HSSSSSSSSH...",
];
const FACE_OPEN = ["...HSESSSSESH...", "...HSSSSSSSSH...", "....SSSMMSSS....", ".....SSSSSS....."];
const FACE_SHUT = ["...HSWWSSWWSH...", "...HSSSSSSSSH...", "....SSSSSSSS....", ".....SSSSSS....."];
const BODY = (a, b) => ["....BBBBBBBB....", "...BBBBBBBBBB...", a, b, "...BBBBBBBBBB...", "...bbbbbbbbbb..."];

const typing1 = [...HEAD, ...FACE_OPEN, ...BODY("..SSBBBBBBBBB...", "...BBBBBBBBSS...")];
const typing2 = [...HEAD, ...FACE_OPEN, ...BODY("...BBBBBBBBSS...", "..SSBBBBBBBBB...")];
// Gio tay vay: tay phai len cao, 2 khung lech nhau 1 pixel.
const wave = (up) => [
  "..............SS",
  ".....HHHHHH...SS",
  "....HHHHHHHH..BB",
  "...HHHHHHHHHH.BB",
  "...HHSSSSSSHH.BB",
  "...HSSSSSSSSHBB.",
  ...FACE_OPEN,
  "....BBBBBBBBBB..",
  "...BBBBBBBBBB...",
  "..SSBBBBBBBBB...",
  "...BBBBBBBBBB...",
  "...BBBBBBBBBB...",
  "...bbbbbbbbbb...",
].map((r, i) => (up || i > 5 ? r : i === 0 ? "................" : r.slice(0, 13) + "." + r.slice(13, 15)));
const idle = [...HEAD, ...FACE_SHUT, ...BODY("...BBBBBBBBBB...", "..SSBBBBBBBBSS..")];

const SKIN = "#f2c9a0";
const PEOPLE = {
  c1: { H: "#3b2a20", B: "#5b7fd6", b: "#46629f" },
  c2: { H: "#e0b04a", B: "#3fa36b", b: "#2f7a50" },
  c3: { H: "#8e3b2e", B: "#d0546f", b: "#a03f55" },
  c4: { H: "#1e1e24", B: "#e39a3b", b: "#b3772b" },
  c5: { H: "#c8c8d0", B: "#8a63c9", b: "#6a4a9c" },
  c6: { H: "#5a3d8a", B: "#2fa3a0", b: "#227a78" },
};

// ---- Do vat ----
const desk = [
  "................",
  "................",
  "................",
  "................",
  "................",
  "................",
  "..........LLLLL.",
  "..........LGGGL.",
  "..........LGGGL.",
  ".........KKKKKKK",
  "TTTTTTTTTTTTTTTT",
  "tttttttttttttttt",
  "PPPPPPPPPPPPPPPP",
  "PppppppppppppppP",
  "PppppppppppppppP",
  "PPPPPPPPPPPPPPPP",
];
const floorWood = [
  "AAAAAAAAAAAAAAAA", "AAAAAAAAAAAAAAAA", "AAAAAAAAAAAAAAAA", "bbbbbbbbbbbbbbbb",
  "CCCCCCbCCCCCCCCC", "CCCCCCbCCCCCCCCC", "CCCCCCbCCCCCCCCC", "bbbbbbbbbbbbbbbb",
  "AAAAAAAAAAAbAAAA", "AAAAAAAAAAAbAAAA", "AAAAAAAAAAAbAAAA", "bbbbbbbbbbbbbbbb",
  "CCCbCCCCCCCCCCCC", "CCCbCCCCCCCCCCCC", "CCCbCCCCCCCCCCCC", "bbbbbbbbbbbbbbbb",
];
const floorCarpet = Array.from({ length: 16 }, (_, y) =>
  Array.from({ length: 16 }, (_, x) => ((x >> 2) + (y >> 2)) % 2 ? "A" : "C").join(""),
);
const wall = [
  "WWWWWWWWWWWWWWWW", "WWWWWWWWWWWWWWWW", "WWWWWWWWWWWWWWWW", "WWWWWWWWWWWWWWWW",
  "wWWWWWWWWWWWWWWw", "WWWWWWWWWWWWWWWW", "WWWWWWWWWWWWWWWW", "WWWWWWWWWWWWWWWW",
  "WWWWWWWWWWWWWWWW", "wWWWWWWWWWWWWWWw", "WWWWWWWWWWWWWWWW", "WWWWWWWWWWWWWWWW",
  "WWWWWWWWWWWWWWWW", "WWWWWWWWWWWWWWWW", "DDDDDDDDDDDDDDDD", "dddddddddddddddd",
];
const window_ = [
  "WWWWWWWWWWWWWWWW", "WFFFFFFFFFFFFFFW", "WFkkkkkkFkkkkkkW", "WFkkkkkkFkkkkkkW",
  "WFkkKkkkFkkkkkkW", "WFkkkkkkFkkKkkkW", "WFFFFFFFFFFFFFFW", "WFkkkkkkFkkkkkkW",
  "WFkkkkkkFkkkkkkW", "WFkkkKkkFkkkkkkW", "WFFFFFFFFFFFFFFW", "WWWWWWWWWWWWWWWW",
  "WWWWWWWWWWWWWWWW", "WWWWWWWWWWWWWWWW", "DDDDDDDDDDDDDDDD", "dddddddddddddddd",
];
const plant = [
  "................", "......GG........", ".....GgGG..G....", "...GGgGGG.GG....",
  "..GGgGGgGGgG....", "...GGGgGGGGG.G..", "....GgGGgGGGGG..", "...GGGGGGgGG....",
  ".....GGgGGG.....", "......GGG.......", ".....PPPPPP.....", ".....pPPPPp.....",
  "......PPPP......", "......pPPp......", "......PPPP......", "................",
];
const shelf = [
  "SSSSSSSSSSSSSSSS", "SrrbbyyrSbbggrrS", "SrrbbyyrSbbggrrS", "SrrbbyyrSbbggrrS",
  "SSSSSSSSSSSSSSSS", "SggyrrbbSyybbrgS", "SggyrrbbSyybbrgS", "SggyrrbbSyybbrgS",
  "SSSSSSSSSSSSSSSS", "SbbrrggySrrbbyyS", "SbbrrggySrrbbyyS", "SbbrrggySrrbbyyS",
  "SSSSSSSSSSSSSSSS", "Ss............sS", "Ss............sS", "SS............SS",
];
const coffee = [
  "................", "................", "....MMMMMMMM....", "....MmmmmmmM....",
  "....MmRRmmmM....", "....MmmmmmmM....", "....MMMMMMMM....", "....M..MM..M....",
  "....M.CCCC.M....", "....M.CwwC.M....", "....M.CCCC.M....", "....MMMMMMMM....",
  "...TTTTTTTTTT...", "...tttttttttt...", "....t......t....", "....t......t....",
];
const lamp = [
  "....YYYYYYYY....", "...YYYYYYYYYY...", "..YYYYYYYYYYYY..", "......llll......",
  ".......MM.......", ".......MM.......", ".......MM.......", ".......MM.......",
  ".......MM.......", ".......MM.......", ".......MM.......", ".......MM.......",
  ".......MM.......", ".....MMMMMM.....", "....MMMMMMMM....", "................",
];
const board = [
  "FFFFFFFFFFFFFFFF", "FwwwwwwwwwwwwwwF", "FwrrrrwwwwbbbbwF", "FwwwwwwwwwwwwwwF",
  "FwbbbbbbbwwwwwwF", "FwwwwwwwwwggggwF", "FwrrwwwwwwwwwwwF", "FwwwwwbbbbbwwwwF",
  "FwwwwwwwwwwwwwwF", "FFFFFFFFFFFFFFFF", ".......FF.......", ".......FF.......",
  "......F..F......", ".....F....F.....", "....F......F....", "................",
];

const files = {};
for (const [id, c] of Object.entries(PEOPLE)) {
  const pal = { ...c, S: SKIN, E: "#2a2a35", W: "#6a4a3a", M: "#c96b6b" };
  files[`char-${id}-typing.svg`] = svg([typing1, typing2], pal);
  files[`char-${id}-waiting.svg`] = svg([wave(true), wave(false)], pal);
  files[`char-${id}-idle.svg`] = svg([idle], pal);
}
files["desk.svg"] = svg([desk], { L: "#3a3d48", G: "#7fd0ff", K: "#9aa0ad", T: "#c08a5a", t: "#9c6b40", P: "#8a5a34", p: "#7a4e2c" });
files["floor-wood.svg"] = svg([floorWood], { A: "#b88a5c", C: "#a87b4f", b: "#8f6640" });
files["floor-carpet.svg"] = svg([floorCarpet], { A: "#5f7a6a", C: "#56705f" });
files["wall.svg"] = svg([wall], { W: "#e8dcc4", w: "#ddd0b6", D: "#9c7a5a", d: "#7d5f44" });
files["window.svg"] = svg([window_], { W: "#e8dcc4", F: "#f7f2e6", k: "#9fd3f0", K: "#ffffff", D: "#9c7a5a", d: "#7d5f44" });
files["plant.svg"] = svg([plant], { G: "#4f9a4a", g: "#3a7a38", P: "#c46a3d", p: "#a2532c" });
files["shelf.svg"] = svg([shelf], { S: "#8a5a34", s: "#6e4628", r: "#c94f4f", b: "#4f7ac9", y: "#e0c04f", g: "#4fa36b" });
files["coffee.svg"] = svg([coffee], { M: "#4a4d58", m: "#2e3038", R: "#e5484d", C: "#f2f2f2", w: "#6b3f22", T: "#c08a5a", t: "#9c6b40" });
files["lamp.svg"] = svg([lamp], { Y: "#f5d36b", l: "#fff3b0", M: "#5a5d68" });
files["board.svg"] = svg([board], { F: "#9aa0ad", w: "#fafafa", r: "#e5484d", b: "#4f7ac9", g: "#3fa36b" });

const slots = {
  wall: "wall.svg",
  desk: "desk.svg",
  "floor.wood": "floor-wood.svg",
  "floor.carpet": "floor-carpet.svg",
  "decor.window": "window.svg",
  "decor.plant": "plant.svg",
  "decor.shelf": "shelf.svg",
  "decor.coffee": "coffee.svg",
  "decor.lamp": "lamp.svg",
  "decor.board": "board.svg",
};
for (const id of Object.keys(PEOPLE)) for (const s of ["typing", "waiting", "idle"]) slots[`character.${id}.${s}`] = `char-${id}-${s}.svg`;

const theme = {
  name: "default",
  font: "\"Cascadia Mono\", Consolas, monospace",
  // characterY/deskY: dich doc (px cua khung) so voi o ngoi, de dau nhan vat lo tren ban.
  sprite: { frame: 32, scale: 2, characterY: -12, deskY: 8 },
  colors: {
    bg: "#1f2128", room: "#2a2d36", border: "#3a3f4a", text: "#e3e3e3", muted: "#8a8f9c", accent: "#d97757",
    review: "#f5a524", waiting: "#e5b567", running: "#7fb77e", idle: "#6b7080", warn: "#e08a3c", "bubble-text": "#1a1a1a",
    error: "#e5484d", shadow: "#000000", input: "#22242b",
  },
  slots,
};
files["theme.json"] = JSON.stringify(theme, null, 2) + "\n";

mkdirSync(OUT, { recursive: true });
for (const [name, body] of Object.entries(files)) writeFileSync(join(OUT, name), body);

// Danh sach file nhung vao Rust (theme mac dinh luon co, ke ca khi office root chua seed).
const names = Object.keys(files).sort();
writeFileSync(
  "src-tauri/src/theme_files.rs",
  "// File sinh tu scripts/gen-theme.mjs, khong sua tay.\n" +
    "pub const DEFAULT_FILES: &[(&str, &str)] = &[\n" +
    names.map((n) => `    ("${n}", include_str!("../office-seed/themes/default/${n}")),`).join("\n") +
    "\n];\n",
);

// Trang xem thu: moi sprite phong 3 lan, co animation dai khung.
const cells = names.filter((n) => n.endsWith(".svg")).map((n) => {
  const frames = (files[n].match(/width="(\d+)"/)[1] | 0) / FRAME;
  const url = "data:image/svg+xml;base64," + Buffer.from(files[n]).toString("base64");
  return `<figure><div class="s" style="--n:${frames};background-image:url(${url})"></div><figcaption>${n}</figcaption></figure>`;
});
const preview = join(tmpdir(), "claudedeck-theme-preview.html");
writeFileSync(preview, `<!doctype html><meta charset="utf-8"><style>
body{background:#1f2128;color:#ccc;font:12px monospace;display:flex;flex-wrap:wrap;gap:16px;padding:16px}
figure{margin:0;text-align:center}.s{width:96px;height:96px;background-size:calc(96px*var(--n)) 96px;image-rendering:pixelated;animation:a .6s steps(var(--n)) infinite;background-color:#2a2d36}
@keyframes a{to{background-position:calc(-96px*var(--n)) 0}}</style>${cells.join("")}`);
console.log(`${names.length} file -> ${OUT}\npreview: ${preview}`);
