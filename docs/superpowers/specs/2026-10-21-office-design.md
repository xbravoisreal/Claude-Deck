# ClaudeDeck Office: thiết kế

Ngày: 2026-10-21. Thay thế `claudedeck-office-plan-v2.md` ở những điểm khác nhau; những gì không nhắc lại ở đây thì theo plan v2.

## 1. Mục đích

ClaudeDeck (Tauri 2 + React, Windows) hiện chạy Claude Code theo tab. Office biến nó thành một "văn phòng": các phòng ban gồm các Agent chuyên trách, chủ app là sếp giao việc và duyệt việc. Giá trị cốt lõi: nhìn một màn là biết Agent nào đang làm, Agent nào đang chờ sếp, và duyệt được cả từ điện thoại.

## 2. Mục tiêu, không làm, phản mục tiêu

**Mục tiêu**
- Màn Office là màn mặc định khi mở app: các phòng ban dạng thẻ, Agent ngồi bàn, trạng thái nhìn là thấy.
- Agent chờ duyệt hiện bóng thoại **"Chờ síp"** trên đầu.
- Nút ☰ góc trên trái bật/tắt danh sách việc chờ duyệt (có huy hiệu đếm).
- Mỗi việc chờ duyệt gửi qua bot Telegram (tên Agent + task + tóm tắt), **duyệt hoặc yêu cầu làm lại ngay trên Telegram**.
- Giao diện pixel-art, đổi được tone màu, bố cục phòng, decor, nền, nhân vật mà không sửa code chức năng.
- Phòng Software Development (4 Agent) và phòng HR (Recruiter) chạy được trọn luồng.

**Không làm (bản này)**
- Template phòng ban, phòng Media Content, prompt router, truy cập từ xa qua Tailscale, routine theo cron, trình chỉnh phòng kéo-thả. Phòng mới vẫn tạo được bằng cách thêm thư mục dữ liệu.
- Duyệt khi máy tắt: bot chỉ phản hồi khi app đang chạy (app thu nhỏ xuống khay).

**Phản mục tiêu (chạy được nhưng vẫn là thất bại)**
- Đổi hình ảnh/theme mà phải sửa component React hay Rust.
- Một task đốt usage gấp nhiều lần tự làm tay (mục tiêu: ~1,5–2 lần một phiên thường).
- Agent tự push, tự xóa hàng loạt, tự tuyển, hoặc Reviewer tự sửa code rồi tự duyệt.
- Bot Telegram nhận lệnh từ người khác ngoài chủ app.

## 3. Ràng buộc và sự thật đã kiểm

- Repo: `H:\01_Coding\claudedeck` (plan v2 ghi sai đường dẫn). Code hiện có ~960 dòng: `pty.rs`, `history.rs`, `usage.rs`, `accounts.rs`, `App.tsx`, `Pane.tsx`, `UsageFooter.tsx`. Không dùng thư viện UI, CSS thuần.
- Claude Code 2.1.306 có: `--agent <name>` (chạy phiên với vai Agent), `--agents <json>` (định nghĩa Agent lúc khởi chạy), `--max-budget-usd`, `--allowedTools`, `--disallowedTools`, `--append-system-prompt`, `--settings`, `--resume`.
- Subagent của Claude Code không gọi được subagent khác; chỉ phiên chính gọi được.
- Đo 39 phiên gần nhất của chủ app: trung vị 29 lượt gọi API, 2,1M token đọc cache, 156K ghi cache, 21K output; p90 rất lệch (44M đọc cache).

## 4. Quyết định của chủ app

- Màn mặc định: Office, bố cục **lưới thẻ phòng (bản A)**, phong cách theo ảnh mẫu (pixel-art, phòng có sàn và decor, nhân vật chibi ngồi bàn).
- Bóng thoại chờ duyệt: chữ **"Chờ síp"**.
- Nút ☰ góc trên trái mở/đóng danh sách chờ duyệt.
- Thông báo: **Telegram**, có nút duyệt/làm lại trên điện thoại.
- Hình ảnh phải thay được dễ dàng, tách khỏi chức năng.
- Software Development khởi đầu **4 Agent**: Tech Lead, Dev, Reviewer, QA.
- Quyền Agent: `--permission-mode auto` + allowlist theo vai + danh sách cấm cứng; lệnh xin quyền duyệt được trên Telegram (mục 5).

## 5. Đội hình

| Agent | Việc | Quyền |
|---|---|---|
| Tech Lead | Nhận task, viết spec ngắn + tiêu chí nghiệm thu, giao cho Dev/Reviewer/QA (subagent), tổng hợp kết quả | đọc, ghi file trong `.office/`, gọi subagent |
| Dev | Làm code (cả frontend lẫn backend) | đọc, ghi, chạy build/test |
| Reviewer | Review diff, chạy test, báo lỗi | chỉ đọc + chạy test, không Edit/Write |
| QA | Viết và chạy test | đọc, ghi file test, chạy test |
| Recruiter (HR) | Tuyển/sửa/cho nghỉ Agent | chỉ ghi vào `candidates/` |

**Quyền (chủ app chọn "cách ở giữa"):** mọi phiên Agent chạy `--permission-mode auto`, không bao giờ `bypassPermissions`/`--dangerously-skip-permissions` (nút Auto-approve hiện có không áp dụng cho phiên Office). Mỗi Agent có:
- `--allowedTools` theo cột Quyền ở bảng trên.
- `--disallowedTools` cứng chung cho mọi Agent: `git push`, xóa hàng loạt (`rm -rf`, `Remove-Item -Recurse`, `del /s`), lệnh mạng ra ngoài (`curl`, `Invoke-WebRequest`, `WebFetch`…). Reviewer thêm cấm `Edit`/`Write`/`NotebookEdit`.
- Lệnh nằm ngoài allowlist mà Claude Code dừng lại hỏi → hook `Notification` báo về → task vào `waiting` với lý do "xin quyền: <lệnh>" → Telegram gửi tin có nút **Cho phép / Từ chối**; app trả lời bằng cách gõ lựa chọn tương ứng vào PTY của phiên. Danh sách chờ duyệt ☰ cũng hiện mục này với hai nút đó.

**Khởi chạy tối giản (giảm token):** đo thực tế, mỗi phiên thường mở đầu với ~57K token ngữ cảnh (system prompt, công cụ, MCP, danh sách skill, hook chèn văn bản), và phần này bị đọc lại ở mọi lượt. Phiên Office luôn chạy với `--strict-mcp-config` (không MCP trừ khi Agent khai báo cần), `--tools` chỉ gồm công cụ của vai, và không nạp plugin/skill ngoài skill của phòng. Mục tiêu: ngữ cảnh mở đầu ≤ 25K. Builder đo con số thật ở Giai đoạn 1 và ghi lại. Đo Giai đoạn 1 (2026-10-21, `claude -p`, Claude Code 2.1.306): Tech Lead 9,4K token, Dev/Reviewer/QA 8–9K token (so với ~57K phiên thường).

**Cảnh báo ngữ cảnh:** app đọc độ dài ngữ cảnh của lượt mới nhất từ transcript. Trên 150K: bàn hiện viền cam + tooltip "phiên dài, nên tách task"; trên 250K: gửi Telegram. Ngưỡng chỉnh trong Settings.

Release và Analyst bị bỏ: spec do Tech Lead viết, release là một skill (`release-checklist`) gọi khi cần.

**Ước lượng chi phí một task đủ luồng:** ~3–4M token đọc cache (~1,5–2 lần một phiên thường). Bản 7 Agent ước ~6–7M (~3 lần). Mỗi lần tuyển qua HR ~1–2 phiên.

## 6. Mô hình dữ liệu

Giữ cấu trúc `office/` của plan v2 (mục 2) với các thay đổi:
- **Không copy vào `.claude/` của dự án đích.** Khi khởi chạy, ClaudeDeck ghi các Agent của phòng thành một plugin tạm (`%TEMP%\claudedeck-plugin-<phòng>`) và truyền qua `--plugin-dir`; phiên chạy bằng `--agent office:<id>`, subagent tên `office:<id>`. (`--agents <file>` chỉ chạy với `--print`, đã thử ở Giai đoạn 1.) Skill vẫn cần nằm ở nơi Claude Code đọc được; cách triển khai skill là việc của builder (xem mục 11).
- Bỏ `meta.json` và `canSpawn`. Frontmatter Agent giữ `name`, `description`, `tools`, `model`. `status` (active/probation/retired) và `budget` nằm trong `department.json`.
- `department.json` có thêm `workspace` (chọn bằng hộp thoại chọn thư mục khi tạo phòng).
- Mỗi phòng có thêm `room.json` (giao diện, xem mục 8).

**Bàn giao giữa các Agent:** mỗi task có thư mục `<workspace>/.office/tasks/<task-id>/` chứa `brief.md` (đề bài + nhận xét làm lại), `spec.md`, `review.md`, `result.md`. Thêm `.office/` vào `.gitignore` của dự án đích.

**Ghi chú trong Obsidian (tùy chọn):** Settings có ô `notesRoot` (đường dẫn vault Obsidian). Có `notesRoot` thì thư mục bàn giao của task chuyển sang `<notesRoot>/ClaudeDeck/<phòng>/<task-id>/` thay vì `<workspace>/.office/tasks/<task-id>/`; phiên Agent được thêm `--add-dir <notesRoot>/ClaudeDeck`. Mỗi phòng có `<notesRoot>/ClaudeDeck/<phòng>/project-notes.md` (kiến trúc, quy ước, quyết định đã chốt), Agent được dặn đọc file này **trước** khi đọc code và Tech Lead cập nhật nó sau mỗi task được duyệt. Mọi file bàn giao có frontmatter `task`, `agent`, `department`, `status`, `created`, `tokens` để Obsidian lọc/thống kê được. Không dùng MCP của Obsidian: Agent đọc ghi file trực tiếp.

**Cách ly:** task có sửa code chạy trong một git worktree riêng (`<workspace>/.office/worktrees/<task-id>`), nhánh `office/<task-id>`. Duyệt xong thì sếp tự merge (không auto-merge, không push).

## 7. Luồng task và trạng thái

Trạng thái task: `backlog → running → waiting (Agent hỏi sếp) → review ("Chờ síp") → done | rework`.

- **Giao việc:** từ màn Office bấm bàn → Giao việc, hoặc giao cho lead phòng. ClaudeDeck mở phiên trong PTY (xem được ở tab Terminal), gắn session id vào task.
- **Phân biệt "xong" và "đang hỏi":** Agent được dặn trong prompt phải ghi `result.md` khi xong. Phiên rảnh và có `result.md` → `review` (Chờ síp). Phiên rảnh mà không có `result.md` → `waiting` (bóng "Chờ anh", thông báo Telegram không có nút).
- **Duyệt:** → `done`. **Làm lại:** nhận xét được ghi thêm vào `brief.md`, phiên được resume với nhận xét làm prompt.
- **Giới hạn song song:** tối đa 3 phiên sống (`running` + `waiting`) cùng lúc, chỉnh trong Settings. Task dư nằm `backlog`.
- **Vòng đời phiên:** task vào `review` thì app đóng PTY (giải phóng chỗ). Làm lại → mở lại bằng `--resume <session-id>`. "Mở phiên" trên task đã đóng = mở tab Terminal với `--resume`.
- **Mỗi Agent một task đang chạy.** Giao thêm cho Agent đang bận → task vào `backlog` của Agent đó, tự chạy khi Agent rảnh. Bàn chỉ vẽ task hiện tại.
- **Trần chi phí:** `--max-budget-usd` chỉ chạy với `--print`, mà phiên Office là phiên tương tác, và chủ app dùng gói subscription. Thay bằng **trần token mỗi task** (`department.json` → `defaults.maxTokens`, mặc định 4M token đọc cache): app cộng usage từ transcript; vượt trần thì gửi Ctrl+C cho phiên, task vào `waiting` kèm lý do "vượt trần", thông báo Telegram.

## 8. Giao diện

**Thanh trên cùng:** nút ☰ (huy hiệu đỏ đếm việc chờ duyệt) · tab Office / Terminal · ô đếm theo trạng thái (Chờ síp, Chờ anh, Đang làm, Rảnh) · thanh usage 5h/1w (chuyển từ footer lên).

**Màn Office:** lưới thẻ phòng, xếp masonry (phòng nhiều bàn chiếm chỗ lớn hơn). Mỗi phòng:
- Tiêu đề phòng + pill trạng thái (vd "1 Chờ síp", "2 Đang làm").
- Sàn/nền và decor theo `room.json`.
- Mỗi Agent một bàn: nhân vật sprite theo trạng thái, bóng thoại trên đầu ("Chờ síp", "Chờ anh", "Đang: <tool đang chạy>"), nhãn dưới bàn (tên task, thời gian).
- Phiên con do Tech Lead gọi: nhóm trong khung viền đứt bên trong phòng.
- Vị trí chưa tuyển: bàn trống viền đứt "+ Tuyển" (mở luồng HR).
- Dòng "Đã xong N" cuối phòng: các thẻ nhỏ task đã duyệt.
- Thẻ cuối lưới: "+ Mở phòng".

**Danh sách chờ duyệt (☰):** trượt ra từ trái, đè lên màn Office. Mỗi mục: tên Agent · phòng, tên task, thời gian, tóm tắt 1–2 dòng từ `result.md`, nút **Duyệt**, **Làm lại** (ô nhận xét), **Mở phiên** (nhảy sang tab Terminal của phiên).

**Bấm vào bàn:** panel chi tiết Agent (vai trò, skill, quyền, model, ngân sách, lịch sử task, nút Giao việc / Sửa / Cho nghỉ).

**Phòng HR:** thêm khu "Ứng viên chờ duyệt" (thẻ hồ sơ: mô tả, quyền xin cấp, kết quả thử việc; nút Nhận / Sửa / Từ chối).

**Tab Terminal:** giữ nguyên giao diện tab hiện tại.

### Theme (tách hình ảnh khỏi chức năng)

- Component chỉ tham chiếu **slot có tên** (vd `floor`, `wall`, `desk`, `character.<state>`, `bubble`, `decor.<id>`) và **token màu** (CSS variables). Không component nào chứa đường dẫn ảnh hay mã màu cụ thể.
- `themes/<tên>/theme.json`: bảng màu (nền, chữ, viền, màu từng trạng thái), font, map slot → file ảnh, kích thước khung sprite.
- Sprite PNG, khung chuẩn 32×32, phóng 2–3 lần, `image-rendering: pixelated`. Nhân vật: mỗi trạng thái một dải khung (gõ phím, giơ tay chờ, rảnh, vắng). Agent chọn nhân vật trong `room.json`; mặc định chọn theo hash của Agent id.
- `departments/<id>/room.json`: kiểu sàn, danh sách decor + vị trí (theo ô lưới của phòng), Agent nào ngồi bàn nào.
- Theme thiếu slot → lấy từ theme mặc định, không lỗi.
- Chọn theme trong Settings. Theme mặc định dựng từ một bộ pixel-art giấy phép CC0 (lựa chọn của Claude; chủ app sẽ thay dần).

## 9. Telegram

- Cài đặt trong Settings: bot token + chat id; nút "Gửi thử".
- ClaudeDeck dùng long-polling `getUpdates`, không mở cổng mạng.
- Khi task vào `review`: gửi "🟠 Chờ síp · *Agent* · *Phòng*\nTask: …\nTóm tắt: …" kèm inline keyboard ✅ Duyệt / ↩ Làm lại. Làm lại → bot hỏi nhận xét, tin nhắn trả lời tiếp theo là nhận xét.
- Khi task vào `waiting`: gửi tin không có nút, chỉ khi vẫn `waiting` sau 45 giây.
- Duyệt trên app → sửa tin Telegram thành "đã duyệt" (bỏ nút). Bấm nút trên một task đã xử lý → bot báo "đã xử lý rồi".
- Chỉ chấp nhận update từ đúng chat id đã cấu hình; bỏ qua mọi nguồn khác. Tin gửi đi chỉ chứa tóm tắt rút gọn, không gửi code.

## 10. Thứ tự giai đoạn

Mỗi giai đoạn chạy được, kiểm được, commit riêng.

1. **Mô hình tổ chức:** thư mục `office/`, đọc `department.json` + Agent, tạo sẵn phòng Software Dev (4 Agent) và HR; khởi chạy phiên Agent bằng `--agent`/`--agents`. *Xong khi:* mở được phiên Tech Lead và Claude Code nhận đúng prompt, công cụ, subagent.
2. **Task và duyệt:** `tasks.json`, thư mục `.office/tasks/<id>/`, worktree, hàng đợi, trạng thái từ transcript + `result.md`, danh sách chờ duyệt ☰. *Xong khi:* trọn luồng giao → làm → Chờ síp → duyệt/làm lại trên một task nhỏ.
3. **Màn Office pixel-art + theme:** lưới phòng, bàn, bóng thoại, `theme.json`, `room.json`, theme mặc định. *Xong khi:* đổi theme hoặc sửa `room.json` thay đổi hình ảnh mà không đụng code.
4. **Telegram:** gửi và duyệt từ điện thoại, thu nhỏ xuống khay. *Xong khi:* duyệt một task từ điện thoại và app cập nhật.
5. **HR:** Recruiter, quy trình 6 bước của plan v2 mục 4, ứng viên chờ duyệt, thử việc, onboard, cho nghỉ. *Xong khi:* tuyển được một Agent mới và giao việc được cho nó.
7. **Mở phòng ban** (plan v2 giai đoạn 6, 2026-10-26): template phòng là dữ liệu ở `<root>/templates/departments/<id>/` (department.json + agents/*.md + room.json; seed sẵn `software-dev`, `media-content`), nút + chọn template hoặc phòng trống; phòng trống có thể "Nhờ HR đề xuất đội hình" (task Recruiter, ứng viên đầu tiên được nhận thành lead). Agent từ template là `active` (template do chủ app giữ). *Xong khi:* mở được phòng Media Content không sửa code.
8. **Routine + chạy cùng Windows** (plan v2 giai đoạn 7, 2026-10-26): routine = task tự giao theo giờ (`departments/<phòng>/routines.json`: `at` "HH:MM", `days` 1–7, rỗng = mỗi ngày), đi qua hàng đợi, worktree, trần token, Chờ síp, Telegram như task thường thay vì runner `claude -p` riêng. App tick 30s; mở app trễ thì giao bù một lần trong ngày; task lần trước chưa xong thì chưa giao thêm. Settings "Chạy cùng Windows" ghi `HKCU\…\Run` với `--hidden` (vào thẳng khay). Khay và Telegram đã có từ giai đoạn 4. **Bỏ:** xem từ xa qua Tailscale (Telegram đã đủ cho điện thoại, không mở cổng mạng), ntfy.
6. ~~Hook~~: **bỏ giai đoạn này.** `pty.rs` đã tiêm hook `Stop`/`Notification` theo từng phiên qua `--settings` về server loopback; giai đoạn 2 dùng luôn tín hiệu đó (không cài hook toàn cục).

**Thử việc HR:** ClaudeDeck (không phải Recruiter) chạy ứng viên bằng `claude -p --agents <json> --agent <id> --max-budget-usd 0.5` trong thư mục nháp `office/hr/probation/<candidate-id>/`. Recruiter ghi task mẫu vào hồ sơ ứng viên (`candidates/<id>/sample-task.md`); kết quả, số lượt, token được ghi vào hồ sơ để sếp xem.

## 11. Để builder tự quyết

- Cách cho Claude Code thấy skill của phòng (`--add-dir`, `--settings`, plugin dir, hay copy vào `.office/`), miễn không ghi vào `.claude/` của dự án đích.
- Schema transcript JSONL và cách suy trạng thái "rảnh"; cách đọc subagent transcript để vẽ phiên con.
- Thư viện cho Telegram (gọi HTTP thẳng từ Rust là đủ) và cho khay hệ thống (plugin tray của Tauri).
- Chọn bộ CC0 cụ thể và layout masonry (CSS columns hay grid).
- Cấu trúc component React và Rust command.

## 12. Các lựa chọn Claude tự đưa ra (chủ app nên rà)

- Truyền Agent qua `--agents` thay vì copy vào `.claude/`.
- `result.md` là tín hiệu "xong" để phân biệt Chờ síp với Chờ anh.
- Mỗi task một git worktree; sếp tự merge.
- Workspace phòng chọn bằng hộp thoại khi tạo phòng.
- Không chạy routine/duyệt khi app đóng.
- Theme mặc định từ bộ CC0; sprite 32×32.
- Thứ tự giai đoạn: tổ chức → task → giao diện → Telegram → HR (hook đã có sẵn, bỏ giai đoạn riêng).
- Trần token mỗi task thay cho trần USD; vượt trần thì dừng phiên và báo.
- Phiên đóng khi vào Chờ síp, mở lại bằng `--resume` khi làm lại.
- Mỗi Agent một task đang chạy, task thêm xếp hàng.
- ClaudeDeck tự chạy thử việc HR bằng `-p` trong thư mục nháp.
- Phiên Office khởi chạy tối giản (không MCP, giới hạn công cụ), cảnh báo ngữ cảnh ở 150K/250K.
- Ghi chú task nằm trong vault Obsidian khi có `notesRoot`; `project-notes.md` mỗi phòng do Tech Lead giữ.
