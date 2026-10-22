# Office Giai đoạn 3: Màn Office pixel-art + theme — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Màn Office thành các phòng pixel-art (sàn, tường, decor, bàn, nhân vật chibi theo trạng thái, bóng thoại), toàn bộ hình ảnh và màu lấy từ theme/room có thể thay mà không sửa code.

**Architecture:** Rust đọc `themes/<tên>/theme.json` (gộp với theme mặc định nhúng sẵn, thiếu slot thì lấy mặc định) và trả về bảng màu + slot → data URL. Mỗi phòng có `room.json` (lưới ô: sàn, decor, chỗ ngồi). React chỉ tham chiếu tên slot và CSS variable. Hình mặc định là SVG pixel-art do một script Node sinh từ lưới ký tự (Claude tự vẽ, phát hành CC0).

**Tech Stack:** Rust (serde_json, không thêm crate), React 18 + TS, CSS thuần, Node script (không thêm package).

**Spec:** `docs/superpowers/specs/2026-10-21-office-design.md` mục 8 (Giao diện, Theme), mục 6 (`room.json`).

## Global Constraints

- Component React và Rust không chứa đường dẫn ảnh hay mã màu cụ thể của theme; chỉ dùng **slot có tên** và **CSS variable** (`--o-*`).
- Slot: `floor.<id>`, `wall`, `desk`, `character.<id>.<state>` (state: `typing`, `waiting`, `idle`), `decor.<id>`. Ảnh nhân vật là dải khung ngang; số khung = rộng / `sprite.frame`.
- `theme.json`: `colors` (map token → màu), `font`, `sprite: { frame, scale }`, `slots` (slot → file tương đối trong thư mục theme).
- Sprite khung 32×32, phóng `scale` lần (mặc định 2), `image-rendering: pixelated`.
- Nhân vật: `room.json` → `seats.<agent>.character`; không có thì chọn theo hash của agent id trên danh sách `character.*` của theme.
- Theme thiếu slot/màu → lấy từ theme mặc định, không lỗi. Theme không tồn tại → dùng mặc định + báo lỗi nhẹ.
- Chọn theme trong Settings (store key `theme`, mặc định `"default"`).
- Chữ bóng thoại: **"Chờ síp"**, **"Chờ anh"**, "Dang: <tool>".

## Review Focus

1. `theme.json` hỏng hoặc thiếu → vẫn render bằng theme mặc định, có thông báo. Test: `theme_broken_falls_back`.
2. Theme người dùng chỉ ghi đè 1 slot → các slot khác vẫn có. Test: `theme_partial_override`.
3. `room.json` thiếu/hỏng hoặc thiếu chỗ ngồi cho một agent → agent vẫn được xếp chỗ tự động. Test: `room_missing_is_default` (Rust) + xếp chỗ tự động trong `Room.tsx`.
4. File ảnh slot trỏ ra ngoài thư mục theme (`../../x`) → bị bỏ qua. Test: `theme_rejects_path_escape`.
5. Ảnh PNG (không chỉ SVG) đọc được. Test: `data_url_png_and_svg`.

---

### Task 1: Rust — theme, room.json, tool hiện tại

**Files:** Create `src-tauri/src/theme.rs`; modify `src-tauri/src/office.rs` (Department có `room`), `src-tauri/src/tasks.rs` (Stats có `tool`), `src-tauri/src/lib.rs`.

**Interfaces — Produces:**
```rust
#[derive(Serialize)] pub struct Theme { pub name: String, pub colors: serde_json::Map<String, Value>, pub font: String,
    pub sprite: Value /* {frame, scale} */, pub slots: BTreeMap<String, String> /* slot -> data URL */, pub errors: Vec<String> }
pub fn load_theme(root: &Path, name: &str) -> Theme   // gop: mac dinh (nhung) <- <root>/themes/<name>
fn data_url(path: &Path) -> Option<String>            // svg/png/gif/webp -> data:<mime>;base64,...; ma hoa base64 tu viet
#[tauri::command] pub fn theme_load(root: String, name: String) -> Theme
#[tauri::command] pub fn theme_list(root: String) -> Vec<String>   // ten thu muc trong <root>/themes, luon co "default"
// office.rs: Department { ..., pub room: Value }  (room.json, thieu/hong -> {} + them loi vao errors neu hong)
// tasks.rs:  Stats { ..., pub tool: String }       (ten tool_use cua message assistant cuoi; "" neu message cuoi khong goi tool)
```
Theme mặc định: file trong `src-tauri/office-seed/themes/default/` nhúng bằng `include_str!` (SVG là text), đồng thời seed ra `<root>/themes/default/` để chủ app có mẫu copy.

- [ ] **Step 1: Test fail:** `theme_default_has_core_slots` (có `wall`, `desk`, `floor.wood`, ≥4 `character.*.typing`, màu `bg`), `theme_partial_override`, `theme_broken_falls_back` (errors không rỗng), `theme_rejects_path_escape`, `data_url_png_and_svg` (PNG 1×1 → bắt đầu `data:image/png;base64,iVBOR`), `room_missing_is_default`, `stats_reports_last_tool`.
- [ ] **Step 2:** `cargo test` — Expected: FAIL.
- [ ] **Step 3:** Cài đặt.
- [ ] **Step 4:** `cargo test` — Expected: PASS hết.
- [ ] **Step 5: Commit** `feat(office): theme loading, room layout and current tool`

### Task 2: Theme mặc định và room.json mẫu

**Files:** Create `scripts/gen-theme.mjs`, `src-tauri/office-seed/themes/default/*` (theme.json + SVG), `src-tauri/office-seed/themes/README.md`, `src-tauri/office-seed/departments/{software-dev,hr}/room.json`; modify `office.rs` (SEED).

- Script: lưới ký tự 16×16 + bảng màu → SVG 32×32 (mỗi ô 2×2, `shape-rendering="crispEdges"`). Nhân vật: một mẫu chibi ngồi bàn, đổi màu tóc/áo ra 6 biến thể; `typing` 2 khung (tay lên/xuống), `waiting` 2 khung (giơ tay vẫy), `idle` 1 khung (ngả ghế). Tile: `floor.wood`, `floor.carpet`, `wall` (có cửa sổ), `desk` (bàn + màn hình). Decor: `plant`, `shelf`, `coffee`, `lamp`, `board`.
- `room.json` mẫu: `{ "cols": 8, "rows": 4, "floor": "floor.wood", "decor": [{"slot":"decor.plant","x":0,"y":0}, ...], "seats": {"tech-lead": {"x":1,"y":1,"character":"character.c1"}, ...} }`. HR dùng `floor.carpet`.
- README themes: cách copy `default` thành theme mới, đổi màu/ảnh, chọn trong Settings.

- [ ] **Step 1:** `node scripts/gen-theme.mjs` — Expected: in danh sách file đã ghi.
- [ ] **Step 2:** Mở `scripts/preview.html` (script sinh luôn) trong trình duyệt, chụp màn hình kiểm tra nhìn được.
- [ ] **Step 3:** `cargo test` — Expected: `theme_default_has_core_slots` PASS, `seed_creates_two_departments` vẫn PASS.
- [ ] **Step 4: Commit** `feat(office): default pixel theme and room layouts`

### Task 3: Giao diện phòng pixel

**Files:** Create `src/theme.tsx` (load theme, đặt `--o-*` lên `.office`, context `slot(name)`), `src/Room.tsx` (phòng + bàn + nhân vật + bóng thoại), `src/AgentPanel.tsx`; modify `src/Office.tsx`, `src/App.tsx`, `src/Settings.tsx`, `src/styles.css`.

Hành vi:
- Lưới phòng: CSS grid `auto-fill, minmax(360px,1fr)`; phòng có > 3 agent chiếm 2 cột. Thẻ cuối "+ Mở phòng" → mở thư mục `<root>/departments` trong Explorer.
- Phòng: tiêu đề + pill đếm trạng thái; khu sàn là lưới `cols × rows` ô (ô = `frame × scale` px), nền lát bằng slot sàn, hàng trên cùng là `wall`; decor đặt theo ô; mỗi agent một bàn tại `seats` (thiếu → ô trống kế tiếp từ trái sang, hàng 1 trở xuống).
- Bàn: `desk` + nhân vật theo trạng thái: `running → typing`, `waiting|review → waiting`, còn lại `idle`; animation `steps(n)` theo số khung. Bóng thoại trên đầu: "Chờ síp" / "Chờ anh" / "Dang: <tool>" (tool lấy từ `task_stats`). Nhãn dưới bàn: tên Agent, tên task + thời gian. Viền cam khi ngữ cảnh ≥ ngưỡng.
- Dòng "Đã xong N" cuối phòng: chip các task done gần nhất (tối đa 5), bấm → mở ☰ tab Lịch sử.
- Bấm bàn → `AgentPanel` (trượt từ phải): mô tả, model, tools, ngân sách (`maxTokens`), lịch sử task của Agent; nút **Giao viec**, **Mo phien**, **Sua** (mở file agent `.md`).
- Thanh trên: sau tab Office/Terminal thêm ô đếm "Chờ síp N · Chờ anh N · Đang làm N".
- Settings: ô chọn theme (`theme_list`), nút "Mo thu muc theme".

- [ ] **Step 1:** `npm run build` — Expected: PASS.
- [ ] **Step 2: Kiểm tay** (sếp): các phòng hiện sàn/tường/decor/nhân vật; giao task → nhân vật gõ phím + "Dang: Edit"; Chờ síp → giơ tay + bóng "Chờ síp".
- [ ] **Step 3: Kiểm theme:** copy `themes/default` → `themes/thu`, đổi `colors.bg` và một ảnh, chọn trong Settings → đổi theo, không sửa code; sửa `room.json` dời một decor → đổi theo sau khi tải lại.
- [ ] **Step 4: Commit** `feat(office): pixel-art rooms driven by theme and room.json`

## Ngoài phạm vi

- Nhóm phiên con (subagent) trong khung viền đứt: cần đọc transcript subagent, để giai đoạn sau.
- Bàn trống "+ Tuyển" (luồng HR) → Giai đoạn 5.
- Chuyển thanh usage 5h/1w từ footer lên thanh trên: giữ footer (có menu tài khoản mở lên trên), làm khi cần.
