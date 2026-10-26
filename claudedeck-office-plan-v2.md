# ClaudeDeck Office v2: công ty AI nhiều phòng ban

Thay thế bản v1. Bản này đổi trọng tâm: không chỉ theo dõi phiên, mà dựng một **văn phòng có tổ chức**. Mỗi phòng ban có các Agent chuyên trách, mỗi Agent có skill riêng. Có phòng HR chuyên tuyển Agent mới, và có thể mở thêm phòng ban bất kỳ về sau.

Dành cho Claude Code đọc và thực thi trong repo `E:\04_CodeWorkspace\claudedeck` (Tauri, Windows).

---

## 1. Ý tưởng

- **Office** gồm nhiều **Department** (phòng ban).
- **Department** gồm nhiều **Agent**, mỗi Agent đảm nhận đúng một vai trò, có prompt, quyền công cụ, model và bộ skill riêng.
- **Phòng đầu tiên:** Software Development.
- **Phòng thứ hai:** HR. Chức năng duy nhất: tuyển Agent mới (và sửa, cho nghỉ việc, đánh giá Agent cũ).
- **Mở rộng:** tạo phòng mới bằng template (ví dụ Media Content) mà không phải sửa code ứng dụng. Phòng mới chỉ là dữ liệu.
- **Bạn là sếp:** giao việc, duyệt việc, duyệt tuyển dụng. Không Agent nào tự kích hoạt Agent khác hoặc tự cấp quyền nguy hiểm khi chưa có chữ ký của bạn.
- **Lớp theo dõi** (v1) vẫn giữ: mỗi phiên đang chạy là một "bàn" có trạng thái Chờ anh, Đang làm, Cần xem, Rảnh, Đã đóng.

Thuật ngữ cần thống nhất:
- **Agent** = định nghĩa một vai trò (file cấu hình, bền vững).
- **Bàn / phiên** = một lần Agent đó đang chạy (session Claude Code, tạm thời).
- **Task** = việc được giao cho một Agent.

---

## 2. Mô hình dữ liệu (dạng file, dễ đọc, dễ git)

Thư mục gốc: `%APPDATA%\claudedeck\office\` (có thể trỏ sang thư mục trong repo để version hóa).

```
office/
  office.json                      # danh sách phòng ban, thứ tự hiển thị
  templates/
    departments/                   # template phòng ban (software-dev, hr, media-content, blank)
    agents/                        # mẫu Agent hay dùng
  departments/
    software-dev/
      department.json              # tên, icon, mục tiêu, thư mục dự án, quy tắc điều phối
      agents/
        tech-lead.md
        frontend-dev.md
        ...
      skills/
        code-review/SKILL.md
        ...
      tasks.json                   # bảng việc của phòng
    hr/
      department.json
      agents/recruiter.md
      skills/create-agent/SKILL.md
      candidates/                  # hồ sơ ứng viên chờ duyệt
  runs/                            # log mỗi lần chạy
```

**Agent** dùng đúng định dạng subagent của Claude Code: file Markdown, phần frontmatter (tên, mô tả khi nào dùng, danh sách công cụ, model) và phần thân là system prompt. **Skill** dùng đúng định dạng `SKILL.md`. Nhờ vậy ClaudeDeck chỉ cần "triển khai" (copy hoặc symlink) các file này vào thư mục `.claude/agents` và `.claude/skills` của dự án đích, Claude Code tự nhận ra.

`department.json` (ví dụ):

```json
{
  "id": "software-dev",
  "name": "Software Development",
  "icon": "💻",
  "mission": "Biến yêu cầu thành code đã review và test.",
  "workspace": "E:\\04_CodeWorkspace\\<du-an>",
  "lead": "tech-lead",
  "agents": ["tech-lead", "frontend-dev", "backend-dev", "code-reviewer", "qa-tester"],
  "defaults": { "maxTurns": 30, "maxBudgetUsd": 3 }
}
```

Mỗi Agent khai báo thêm (trong file riêng `agents/<id>.meta.json` hoặc frontmatter mở rộng): `tools` (allowlist), `model`, `skills`, `budget`, `canSpawn` (Agent nào được gọi), `status` (active, probation, retired).

---

## 3. Phòng ban 1: Software Development

Mỗi Agent một việc, có skill riêng. Đội hình đã chốt: 4 Agent (HR có thể chỉnh sau):

| Agent | Việc duy nhất | Skill đi kèm | Quyền chính |
|---|---|---|---|
| Tech Lead | Nhận yêu cầu, chia việc, điều phối, tổng hợp kết quả | task-breakdown, handoff-report | đọc, gọi Agent khác |
| Dev | Làm giao diện, API, logic, dữ liệu | ui-conventions, api-conventions | đọc, ghi, chạy build/test |
| Code Reviewer | Review diff, tìm lỗi và rủi ro | code-review-checklist | chỉ đọc |
| QA Tester | Viết và chạy test, báo lỗi tái hiện được | test-writing | đọc, ghi test, chạy test |

Luồng chuẩn: Bạn giao task cho Tech Lead làm rõ và chia việc -> Dev thực hiện -> Reviewer và QA kiểm tra -> Tech Lead tổng hợp -> **Bạn duyệt**. Reviewer và QA phải ở trạng thái chỉ đọc hoặc quyền hẹp để không "tự sửa rồi tự khen".

Hai chế độ chạy, cho phép chọn theo task:
- **Ủy quyền qua Tech Lead:** một phiên Tech Lead gọi các Agent khác như subagent. Dễ theo dõi, các Agent con hiện thành bàn phụ của phiên cha.
- **Giao thẳng Agent:** mở một phiên riêng cho một Agent (ví dụ chỉ nhờ Code Reviewer xem một nhánh). Cách khởi chạy cụ thể (cờ CLI hay prompt chỉ định subagent) cần xác nhận ở mục 0.

---

## 4. Phòng ban 2: HR (tuyển Agent)

HR có đúng một Agent chính là **Recruiter**, kèm skill `create-agent`. Quy trình tuyển:

1. **Nhận yêu cầu tuyển dụng.** Từ bạn ("cần Agent viết changelog"), hoặc từ Tech Lead ("thiếu người làm migration DB") dưới dạng đề xuất chờ bạn xác nhận.
2. **Phỏng vấn nhu cầu.** Recruiter hỏi lại các điểm còn mơ hồ: việc cụ thể, đầu vào và đầu ra, phòng nào, công cụ nào cần, mức rủi ro.
3. **Soạn hồ sơ ứng viên.** Sinh file Agent (`.md`), các skill cần thiết, `meta` (quyền, model, ngân sách). Nguyên tắc: quyền tối thiểu, một Agent một việc, mô tả "khi nào dùng" rõ ràng để việc điều phối đúng.
4. **Thử việc (probation).** Chạy Agent trên một task mẫu trong thư mục nháp, ghi lại kết quả, số lượt, token.
5. **Báo cáo cho sếp.** Hồ sơ ứng viên hiện ở phòng HR dạng "ứng viên chờ duyệt": mô tả, quyền xin cấp, kết quả thử việc. Bạn bấm **Nhận** hoặc **Sửa** hoặc **Từ chối**.
6. **Onboard.** Chỉ sau khi bạn nhận: ghi file vào `departments/<phòng>/agents`, cập nhật `department.json`, triển khai vào `.claude/agents` của dự án, Agent chuyển trạng thái `active`.

HR còn làm: sửa Agent (cập nhật prompt, skill), cho nghỉ việc (`retired`, không xóa file), và **đánh giá định kỳ** từ dữ liệu có sẵn (số phiên, số lần phải làm lại, token, tỷ lệ bị bạn từ chối).

Ràng buộc an toàn cho HR (bắt buộc):
- Recruiter chỉ được ghi vào thư mục `candidates/` cho tới khi bạn duyệt.
- Công cụ nguy hiểm (xóa file hàng loạt, git push, mạng ngoài, chi tiền) không bao giờ nằm trong quyền mặc định của Agent mới. Nếu cần phải được ghi rõ trong hồ sơ và bạn đồng ý riêng.
- Agent mới luôn bắt đầu ở `probation`, có ngân sách thấp hơn.

---

## 5. Mở phòng ban mới (ví dụ Media Content)

Nút **Mở phòng ban** trong giao diện, hai cách:
- **Từ template:** chọn `software-dev`, `media-content`, `blank`. Template là file JSON mô tả tên, mục tiêu và đội hình đề xuất.
- **Nhờ HR dựng:** bạn mô tả mục tiêu phòng ("sản xuất nội dung video ngắn"), Recruiter đề xuất đội hình và tuyển lần lượt từng Agent theo quy trình mục 4.

Ví dụ template Media Content: Content Strategist (ý tưởng, lịch đăng), Scriptwriter (kịch bản), Video Editor (dựng, phụ đề), Social Poster (caption, đăng bài, chỉ khi bạn duyệt), Analyst (báo cáo hiệu quả). Mỗi vai trò có skill riêng, workspace riêng.

Thêm phòng mới = thêm một thư mục dữ liệu và một thẻ phòng trên giao diện. Không đổi code Rust hay frontend.

---

## 6. Task board và giao việc

- `tasks.json` mỗi phòng: `id`, tiêu đề, mô tả, Agent được giao, trạng thái (`backlog`, `assigned`, `running`, `review`, `done`, `rejected`), phiên liên kết, kết quả tóm tắt.
- **Giao việc:** kéo task vào một Agent hoặc gửi cho lead của phòng. ClaudeDeck mở phiên tương ứng và gắn session id vào task.
- **Duyệt việc:** khi phiên kết thúc, task chuyển `review`. Bạn xem tóm tắt, bấm Duyệt hoặc Làm lại kèm nhận xét (nhận xét được đưa lại làm prompt cho lần sau, và được HR dùng khi đánh giá).
- **Hàng đợi:** giới hạn số phiên chạy song song (mặc định 3 đến 5) để không đốt usage. Task dư nằm chờ.

---

## 7. Lớp theo dõi (giữ từ v1, tóm tắt)

- **Nguồn dữ liệu hợp nhất:** PTY của phiên trong ClaudeDeck, hook của Claude Code gửi về HTTP local `127.0.0.1`, và transcript JSONL trong `~/.claude/projects`.
- **State engine (Rust):** hook mới hơn thì tin hook; không thì suy từ transcript. Trạng thái: waiting, working, review, idle, closed. Phát sự kiện Tauri cho frontend khi đổi.
- **Cài hook:** nút trong Settings, sao lưu `~/.claude/settings.json`, gộp (không ghi đè), đánh dấu định danh để gỡ sạch được.
- **Thông báo:** ntfy hoặc Telegram, chỉ gửi nếu phiên vẫn `waiting` sau 45 giây; gửi thêm khi routine lỗi.
- **Routine:** `claude -p` theo cron, prompt qua stdin, luôn có giới hạn lượt, ngân sách (nếu CLI hỗ trợ), timeout, allowlist công cụ. Routine thuộc một Agent cụ thể của một phòng.

---

## 8. Giao diện

- **Màn Office:** lưới thẻ phòng ban. Mỗi thẻ là một "căn phòng" có bàn cho từng Agent. Agent đang chạy: nhân vật ngồi bàn, hoạt ảnh theo trạng thái. Agent không chạy: nhân vật mờ. **Vị trí chưa tuyển: bàn trống** (gợi ý trực quan, bấm vào để nhờ HR tuyển).
- **Phòng HR:** có khu "Ứng viên chờ duyệt" (thẻ hồ sơ, nút Nhận, Sửa, Từ chối) và nút "Tuyển Agent mới".
- **Chi tiết Agent (bấm vào bàn):** vai trò, skill, quyền, model, ngân sách, lịch sử phiên, thống kê, nút Giao việc, Sửa, Cho nghỉ việc.
- **Chi tiết phiên:** tin nhắn cuối, nút Mở phiên, Đã duyệt, sao chép lệnh resume.
- **Nút Mở phòng ban** và tab **Task** và tab **Routine**.
- Sprite SVG hoặc CSS thuần, sinh từ hash của Agent id (mỗi Agent có diện mạo cố định, dễ nhận ra).

---

## 9. Lộ trình triển khai

Mỗi giai đoạn chạy được, kiểm tra được và commit riêng.

**Mục 0 (trước khi code):** đọc repo (stack frontend, chỗ xử lý PTY, nguồn footer usage). Chạy `claude --help` xác nhận các cờ: `-p`, `--max-turns`, `--allowedTools`, `--resume`, `--output-format`, có `--max-budget-usd` hay không, và cách chỉ định một subagent khi khởi chạy phiên. Đọc docs hiện hành về định dạng subagent, `SKILL.md` và hook. Xem một transcript JSONL thật để chốt schema. Báo lại, chưa viết code.

**Giai đoạn 1: Lớp theo dõi.** Parser transcript, watcher, state engine từ transcript, tab Văn phòng bản thô (danh sách phiên theo dự án). *Xong khi:* mở 3 phiên thấy đúng trạng thái.

**Giai đoạn 2: Mô hình tổ chức.** Cấu trúc thư mục `office/`, đọc và ghi `department.json`, Agent, skill; "triển khai" Agent và skill vào `.claude/` của dự án; màn Office hiện phòng và bàn theo dữ liệu. Tạo sẵn phòng Software Development với 7 Agent mục 3. *Xong khi:* từ ClaudeDeck mở được phiên cho từng Agent và Claude Code nhận đúng prompt và skill.

**Giai đoạn 3: Task board và giao việc.** `tasks.json`, giao việc cho lead hoặc Agent, hàng đợi song song, màn duyệt việc, ghi nhận xét làm lại. *Xong khi:* chạy trọn luồng giao -> làm -> review -> duyệt trên một task thật nhỏ.

**Giai đoạn 4: Hook và độ chính xác.** HTTP server local, script hook, nút Cài/Gỡ hook, hợp nhất hook với transcript và PTY. *Xong khi:* waiting và review hiện trong dưới 1 giây.

**Giai đoạn 5: Phòng HR.** Recruiter + skill `create-agent`, quy trình 6 bước mục 4, màn ứng viên chờ duyệt, thử việc, onboard, cho nghỉ việc. *Xong khi:* nhờ HR tuyển một Agent mới (ví dụ viết changelog), duyệt, và Agent đó chạy được ở Software Development.

**Giai đoạn 6: Mở phòng ban.** Template phòng, nút Mở phòng ban, luồng nhờ HR dựng phòng, template Media Content. *Xong khi:* mở được phòng Media Content mà không sửa dòng code ứng dụng nào.

**Giai đoạn 7: Routine, thông báo, từ xa.** Scheduler, runner có giới hạn, chạy cùng Windows và thu nhỏ xuống khay, ntfy hoặc Telegram, xem từ điện thoại qua Tailscale (bắt buộc token nếu mở ra mạng).

**Bổ sung sau GĐ8 (đã làm):** xem/duyệt từ xa qua Tailscale (Settings, cổng 7878, token trong Credential Manager, chỉ bind IP Tailscale); tab Task và Routine ở thanh trên; hook gắn theo từng phiên qua `--settings` tạm nên không có nút Cài/Gỡ hook (không đụng `~/.claude/settings.json`). Routine chỉ chạy khi app mở (kể cả dưới khay), chưa dùng Windows Task Scheduler.

**Giai đoạn 8: Thống kê và prompt router.** Token và chi phí theo phòng và theo Agent, báo cáo đánh giá cho HR, prompt router chọn Haiku, Sonnet hoặc Opus theo từng Agent và loại task (bắt đầu bằng luật đơn giản, đo rồi mới chỉnh).

---

## 10. Rủi ro và quyết định cần chốt

- **Chất lượng Agent do prompt quyết định.** Mỗi Agent phải có mô tả hẹp và ví dụ đầu ra mong đợi; HR phải thử việc trước khi nhận.
- **Chi phí:** nhiều Agent chạy song song đốt usage rất nhanh. Giới hạn song song, lượt, ngân sách và timeout là bắt buộc.
- **Quyền hạn:** quyền tối thiểu theo vai trò; Reviewer và QA không có quyền sửa code nguồn; Agent nào cũng không tự push hay xóa hàng loạt khi chưa có duyệt.
- **Tự tuyển lan man:** Agent không được tự yêu cầu tuyển ngoài hình thức "đề xuất chờ duyệt".
- **Schema transcript và định dạng subagent có thể đổi** giữa các phiên bản Claude Code; parser chịu lỗi, có test với file mẫu thật.
- **Waiting suy từ transcript có thể nhầm** với tool chạy lâu; hook `Notification` là nguồn chính.
- **Bảo mật:** hook server chỉ nghe loopback; thông báo ra ngoài chỉ gửi nội dung rút gọn.
- **Cần bạn chốt:** (1) workspace mặc định của phòng Software Development là dự án nào, (2) đội hình 7 Agent có cần bớt, (3) ntfy hay Telegram, (4) routine có cần chạy khi app đóng (nếu có thì thêm phương án Windows Task Scheduler).

---

## 11. Cách giao cho Claude Code trong ClaudeDeck

1. "Đọc `claudedeck-office-plan-v2.md`. Làm mục 0, báo lại kết quả, chưa viết code."
2. "Làm Giai đoạn 1." Dừng, bạn kiểm tra.
3. Lặp lại cho từng giai đoạn, mỗi giai đoạn một commit. Nếu muốn thấy giá trị sớm, đổi thứ tự: làm Giai đoạn 2 và 5 trước (tổ chức và HR), để Giai đoạn 4 (hook) sau.
