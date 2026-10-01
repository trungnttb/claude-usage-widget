# Hai nguồn dữ liệu

App không gọi mạng. Mọi số đều lấy từ máy của người dùng, qua đúng hai đường.

Mọi con số trong file này là đo thật trên máy dev ngày 2026-09-10, không phải
ước lượng. Cách đo ghi kèm từng mục để kiểm lại được.

---

## Nguồn 1 — transcript JSONL (mặc định, luôn bật)

`~/.claude/projects/<project-slug>/<session-id>.jsonl`

Mỗi dòng là một bản ghi JSON. Dòng nào có `message.usage` thì đếm được.

### Trường dùng tới

| Trường | Dùng để |
|---|---|
| `message.usage.input_tokens` | token input thật (không tính cache) |
| `message.usage.output_tokens` | token output |
| `message.usage.cache_read_input_tokens` | token đọc lại từ cache |
| `message.usage.cache_creation.ephemeral_5m_input_tokens` | ghi cache TTL 5 phút |
| `message.usage.cache_creation.ephemeral_1h_input_tokens` | ghi cache TTL 1 giờ |
| `message.usage.speed` | `standard` hoặc `fast`, đổi bảng giá |
| `message.model` | định danh model |
| `message.id` + `requestId` | khóa khử trùng lặp |
| `timestamp` | gom theo ngày, theo cửa sổ |
| `cwd` | gom theo project |
| `sessionId` | gom theo phiên |

Không đọc `message.content`. Đó là nội dung hội thoại, app không cần.

### Khử trùng lặp là bắt buộc

Một API response ghi ra **nhiều dòng JSONL cùng mang một khối `usage`** — mỗi
content block một dòng (thinking, tool_use...), nhưng khối `usage` bị lặp
nguyên vẹn ở tất cả các dòng đó.

Đo trên 126 file:

```
giữ lại   = 5539 bản ghi
bỏ đi     = 5286 bản trùng  (48.8% số dòng có usage)
```

Không khử thì mọi con số bị gấp khoảng đôi. Khóa: `message.id` + `":"` + `requestId`.

### Hiệu năng

Đo bằng bản Rust thật (`cargo test --release -- --ignored real_directory`):

```
126 file · 5539 bản ghi sau khử trùng lặp · 5286 bản trùng bị bỏ
quét toàn bộ: 207 ms
```

**Cẩn thận với `opt-level` trong `Cargo.toml`.** Ban đầu để `opt-level = "s"`
cho bản cài nhỏ, quét mất **1386 ms**. Đổi sang `opt-level = 3` còn **207 ms**,
nhanh gấp 6.5 lần. Vòng lặp parse này chạy lúc khởi động và lặp lại mỗi chu kỳ
đọc, nên không đánh đổi tốc độ lấy vài trăm KB. Binary bản release: 7.9 MB.

Lần đầu mở app quét toàn bộ. Sau đó nhớ `(đường dẫn, kích thước, offset)` từng
file, chỉ đọc phần mới ghi thêm. Dòng cuối có thể bị cắt giữa chừng khi Claude
Code đang ghi — giữ lại phần dư, ghép với lần đọc sau.

### Quy đổi tiền

Bảng giá USD trên 1 triệu token (bản chốt 2026-06-24):

| Model | input | output |
|---|---|---|
| `claude-fable-5-1`, `claude-fable-5` | 10 | 50 |
| `claude-opus-5`, `claude-opus-4-8`, `claude-opus-4-7`, `claude-opus-4-6` | 5 | 25 |
| `claude-sonnet-5` | 2 | 10 |
| `claude-sonnet-4-6` | 3 | 15 |
| `claude-haiku-4-5` | 1 | 5 |

Hệ số cache, nhân với giá input của chính model đó:

| Loại | Hệ số |
|---|---|
| đọc cache | 0.1 (riêng `claude-fable-5-1`: 0.025) |
| ghi cache TTL 5 phút | 1.25 |
| ghi cache TTL 1 giờ | 2.0 |

Fast mode (`speed: "fast"`, chỉ Opus 5 và Opus 4.8) tính giá 10 / 50.

Định danh model trong transcript có phần đuôi mà bảng giá không có:
`claude-opus-5[1m]`, `claude-haiku-4-5-20251001`. Phải cắt `[...]` và đuôi ngày
trước khi tra bảng. Gặp model lạ thì tính theo mức Opus và **đánh dấu là ước
lượng** trên giao diện, không im lặng bỏ qua.

### Tỉ trọng cache

Đo trên toàn bộ lịch sử, riêng `claude-opus-5`:

```
input thật    8 639 token
đọc cache   857 403 432 token
ghi cache    17 266 653 token
```

Đọc cache chiếm khoảng 99% lượng input. Gộp chung input với cache read thì con
số sai vài bậc. Đây cũng là lý do có thẻ "cache hit rate" trên giao diện: tỉ lệ
này tụt là dấu hiệu có gì đó đang phá cache và làm tốn tiền vô ích.

Tổng toàn bộ lịch sử nếu tính theo giá API: **$1043.92**.

---

## Nguồn 2 — `claude -p "/usage"` (bật mặc định, tắt được)

Đây là đường duy nhất lấy được **phần trăm hạn mức thật từ server**. Transcript
local không chứa thông tin này.

```bash
claude -p "/usage" --output-format json
```

Trả về JSON, phần cần nằm ở `.result` dưới dạng văn bản thuần:

```
You are currently using your subscription to power your Claude Code usage

Current session: 41% used · resets Sep 10, 2:40pm (Asia/Ho_Chi_Minh)
Current week (all models): 23% used · resets Sep 15, 4am (Asia/Ho_Chi_Minh)
Current week (Fable): 22% used · resets Sep 15, 4am (Asia/Ho_Chi_Minh)
```

### Không tốn token

Đo từ chính JSON trả về:

```
total_cost_usd   0
num_turns        0
duration_api_ms  0
usage.*          tất cả bằng 0
```

Lệnh này không gọi model. Chỉ tốn thời gian khởi động CLI.

### Chi phí thời gian

~2.9 giây mỗi lần chạy, gần như toàn bộ là khởi động CLI chứ không phải chờ
mạng. Vì vậy nhịp mặc định là **3 phút**, không phải vài giây.

### Ba dòng cần bắt

| Dòng | Ý nghĩa |
|---|---|
| `Current session: N% used · resets <thời điểm>` | cửa sổ 5 tiếng đang chạy |
| `Current week (all models): N% used · resets <thời điểm>` | hạn mức tuần chung |
| `Current week (<Tên model>): N% used · resets <thời điểm>` | hạn mức tuần riêng của một model |

Dòng thứ ba có tên model thay đổi (`Fable`, có thể là tên khác) nên phải bắt
bằng nhóm chữ tự do, không hard-code `Fable`.

### Điểm dễ vỡ

App phụ thuộc vào câu chữ tiếng Anh trong output. Anthropic sửa câu là parser
hỏng. Cách chống:

1. Parser trả `None` cho từng dòng không khớp, không làm hỏng cả app.
2. Khi cả ba dòng đều không khớp: giao diện hiện trạng thái "không đọc được
   `/usage`", vẫn chạy tiếp bằng số local, không hiện số cũ như thể còn mới.
3. Giữ lần đọc thành công gần nhất kèm thời điểm, hiện kèm ghi chú "số lúc HH:MM".

### Lệnh này chạy trong shell nào

Trên Git Bash, chuỗi `/usage` bị MSYS2 đổi thành đường dẫn Windows trước khi tới
CLI. App sinh tiến trình trực tiếp (không qua shell) nên không dính. Nhưng khi
kiểm bằng tay thì phải dùng PowerShell, hoặc đặt `MSYS_NO_PATHCONV=1`.

### App tìm file `claude` ở đâu

Không tra PATH thừa hưởng là xong. App đóng gói trên macOS do launchd mở nên
nhận PATH của launchd, trong đó không có chỗ nào mà installer của Claude Code
ghi vào — `claude` chạy tốt trong terminal mà app vẫn báo không tìm thấy.

App tìm theo bốn lớp, dừng ở lớp đầu tiên có kết quả: ô `claudePath` trong cấu
hình → PATH thừa hưởng → các chỗ cài thông dụng → hỏi login shell (tối đa 5
giây). Lý do và ràng buộc: ADR 0004.

Người dùng gặp lỗi `command-not-found` thì chạy `which claude` trong terminal,
dán đường dẫn vào ô "Đường dẫn tới lệnh `claude`" ở cửa sổ cấu hình.

### Lệnh chạy trong thư mục nào

Trong `<thư mục cấu hình>/usage-cwd`, một thư mục rỗng của riêng app.

Không để nó thừa hưởng cwd của app: app đóng gói mở từ Finder có cwd là `/`, mà
CLI coi cwd là project đang mở, nên từ `/` nó đọc vào mọi thư mục được macOS bảo
vệ trong home — người dùng bị hỏi xin quyền `Desktop`, `Music`, `Documents`
từng cái một, dưới tên app.

Kèm theo, cần biết: mỗi lần poll CLI ghi một session file khoảng 2.4 KB vào
`~/.claude/projects/<slug của cwd>/`. Không có `usage` block nên không làm sai
số tiền, nhưng chúng dồn lại đúng trong thư mục mà scanner đang duyệt.

---

## Cái không lấy được

| Muốn | Vì sao không |
|---|---|
| Usage của thiết bị khác, hoặc của claude.ai | `/usage` chỉ tổng hợp từ session trên máy này; phần trăm hạn mức thì là số server nên đã gồm tất cả |
| Số tiền Anthropic thật sự trừ | Không có với tài khoản subscription. Số app hiện là "nếu tính theo giá API", không phải tiền bị trừ |
| Usage & Cost Admin API | Cần admin key và **không dùng được cho tài khoản cá nhân**. Đã bỏ khỏi phạm vi, xem ADR 0002 |
| Đọc token trong `.credentials.json` để gọi endpoint nội bộ | Cấm, xem ADR 0003 |
