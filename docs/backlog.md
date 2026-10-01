# Backlog

Hàng đợi công việc. Làm theo thứ tự phụ thuộc. Xong một mục thì đổi trạng thái
của chính mục đó **trong cùng thay đổi code** — backlog nói khác code thì tệ hơn
là không có backlog.

Trạng thái: `todo` · `doing` · `done` · `blocked`

Cổng nghiệm thu chung cho mọi mục, phải chạy thật:
`cargo fmt --check` · `cargo clippy -- -D warnings` · `cargo test`

---

## Tình hình

| Nhóm | Xong | Tổng |
|---|---|---|
| E1 Nền móng | 3 | 3 |
| E2 Nguồn transcript | 5 | 5 |
| E3 Nguồn `/usage` | 5 | 5 |
| E4 Dự đoán | 1 | 1 |
| E5 Giao diện widget | 6 | 6 |
| E6 Khay và taskbar | 4 | 5 |
| E7 Cấu hình | 5 | 5 |
| E8 Cảnh báo | 1 | 1 |
| E9 Đóng gói | 3 | 4 |
| **Tổng** | **33** | **35** |

Gate chạy lần gần nhất trên toàn repo, trên macOS 15 ARM: `cargo fmt --check`
sạch · `cargo clippy --all-targets -- -D warnings` 0 cảnh báo · `cargo test`
130/130 pass. 4 mục `ignored` là chủ ý: 2 mục in ra cho người xem, 2 mục gọi CLI
thật. Một mục nữa chỉ chạy ngoài macOS (`the_icon_takes_only_what_it_can_draw`)
và đã chạy riêng trong cấu hình đó.

Hai mục `doing` còn lại đều chờ người mở app trên macOS xem bằng mắt, không phải
chờ code.

---

## E1 — Nền móng

### CUW-001 · Dựng khung Tauri chạy được · done

Phụ thuộc: không

`Cargo.toml`, `tauri.conf.json`, `build.rs`, `main.rs`, một cửa sổ trống.

Nghiệm thu:
- `npm run tauri dev` mở được cửa sổ
- `cargo clippy -- -D warnings` sạch
- Bật feature `tray-icon` trong `Cargo.toml`

### CUW-002 · Khung Snapshot và đường phát sự kiện · done

Phụ thuộc: CUW-001

Định nghĩa `Snapshot` theo `docs/architecture.md`, giữ trong `Mutex` dùng chung,
phát sự kiện `snapshot` mỗi khi đổi. Frontend nhận và in ra console.

Nghiệm thu:
- Frontend nhận được sự kiện với dữ liệu giả
- Mọi trường `Option` đều xử lý được trường hợp rỗng

### CUW-003 · Vòng lặp thu thập · done

Phụ thuộc: CUW-002, CUW-014, CUW-030, CUW-060

Nối hai nguồn vào một `Snapshot`. Hai luồng độc lập, mỗi luồng nhịp riêng, cùng
dựng lại snapshot và phát sự kiện xuống giao diện.

Mục này thêm vào sau khi đã dựng xong E2 và E3. Lúc lập backlog ban đầu bỏ sót
phần nối, tưởng nó nằm trong CUW-002 — thực ra CUW-002 chỉ dựng đường ống rỗng.

Nghiệm thu:
- Một nguồn hỏng không làm dừng nguồn kia
- Luồng `/usage` tôn trọng nhịp giãn dần của CUW-022
- Tắt `/usage` trong cấu hình thì luồng đó không chạy lệnh
- Dựng snapshot không giữ khóa lâu tới mức chặn giao diện

---

## E2 — Nguồn transcript

### CUW-010 · Bảng giá và phép tính tiền · done

Phụ thuộc: CUW-001

`pricing.rs`: bảng giá, hệ số cache, cắt đuôi định danh model, tính tiền.
Số liệu lấy từ `docs/data-sources.md`.

Nghiệm thu:
- `claude-opus-5[1m]` và `claude-haiku-4-5-20251001` cắt đúng
- Model ngoài bảng trả về cờ báo là ước lượng, không im lặng
- `speed: "fast"` dùng bảng giá 10/50
- `claude-fable-5-1` dùng hệ số đọc cache 0.025, không phải 0.1
- Test: input 2, output 268, đọc cache 33316, ghi cache 1h 25263 trên
  `claude-opus-5` ra `0.275998` USD, sai số dưới 1e-6

### CUW-011 · Quét và khử trùng lặp · done

Phụ thuộc: CUW-010

Duyệt `~/.claude/projects/**/*.jsonl`, đọc dòng có `message.usage`, khử trùng
lặp theo `message.id` cộng `requestId`.

Nghiệm thu:
- Trên fixture có bản ghi lặp, số sau khử đúng như mong đợi
- Dòng JSON hỏng bị bỏ qua, không làm dừng cả lần quét
- Bản ghi model tổng hợp không tính vào tiền
- Quét toàn bộ thư mục thật dưới 1 giây

### CUW-012 · Đọc theo kiểu ghi thêm · done

Phụ thuộc: CUW-011

Nhớ đường dẫn, kích thước và vị trí đọc dở của từng file, lần sau chỉ đọc phần
mới ghi thêm.

Nghiệm thu:
- Ghi thêm vào file fixture rồi quét lại: chỉ đọc phần thêm
- Dòng cuối bị cắt giữa chừng được giữ lại và ghép đúng ở lần đọc sau
- File bị xóa hoặc thu nhỏ thì quét lại từ đầu file đó, không hỏng

### CUW-013 · Gom nhóm · done

Phụ thuộc: CUW-012

Gom theo ngày cho 14 ngày gần nhất, theo model, theo project lấy từ `cwd`, và
theo phiên đang chạy.

Nghiệm thu:
- Gom theo ngày dùng múi giờ máy, không phải UTC
- Project lấy tên thư mục cuối của `cwd`
- Phiên đang chạy là `sessionId` có bản ghi mới nhất trong 30 phút

### CUW-014 · Chỉ số dẫn xuất · done

Phụ thuộc: CUW-013

Trung bình 7 ngày, tỉ lệ hôm nay so với trung bình, và cache hit rate tính bằng
đọc cache chia cho tổng đọc cache cộng ghi cache cộng input.

Nghiệm thu:
- Trung bình 7 ngày bỏ qua ngày không có dữ liệu, không tính ngày đó là 0
- Chia cho 0 không sinh giá trị không hợp lệ xuống frontend

---

## E3 — Nguồn `/usage`

### CUW-020 · Chạy lệnh và lấy kết quả · done

Phụ thuộc: CUW-002

Sinh tiến trình `claude -p "/usage" --output-format json` trực tiếp, không qua
shell. Lấy trường `result`.

Nghiệm thu:
- Không qua shell nên không dính lỗi đổi đường dẫn của Git Bash
- Quá 30 giây thì bỏ, không treo
- Không tìm thấy lệnh `claude` thì báo lỗi rõ ràng, app vẫn chạy
- Chạy nền, không chặn giao diện

### CUW-021 · Bóc tách ba dòng hạn mức · done

Phụ thuộc: CUW-020

Bắt ba dòng theo `docs/data-sources.md`, lấy phần trăm và thời điểm reset.

Nghiệm thu:
- Tên model ở dòng thứ ba bắt bằng nhóm chữ tự do, không viết cứng `Fable`
- Thời điểm reset đổi được sang mốc thời gian tuyệt đối, xử lý cả trường hợp
  reset rơi sang ngày hôm sau
- Dòng nào không khớp thì trả rỗng, không làm hỏng hai dòng còn lại
- Test chạy trên fixture `usage-output.txt`, không gọi CLI thật

### CUW-022 · Xử lý khi đọc hỏng · done

Phụ thuộc: CUW-021

Giữ lần đọc thành công gần nhất kèm thời điểm. Ba trạng thái theo `docs/ui-spec.md`.

Nghiệm thu:
- Lần chạy hỏng không xóa số đọc được trước đó
- `limits_read_at` luôn có giá trị khi `limits` có giá trị
- Hỏng liên tiếp 3 lần thì giãn nhịp thử lại, không chạy lệnh 3 phút một lần mãi

### CUW-023 · Tìm lệnh `claude` khi app không nhận PATH của shell · done

Phụ thuộc: CUW-020, CUW-060

App đóng gói trên macOS do launchd mở, không do shell mở, nên PATH nó nhận là
PATH của launchd — không có chỗ nào mà installer của Claude Code ghi vào. Chi
tiết và lý do chọn cách này: `docs/decisions/0004-tim-lenh-claude.md`.

Tìm theo bốn lớp, dừng ở lớp đầu tiên có kết quả: đường dẫn user tự điền trong
cấu hình → PATH thừa hưởng → các chỗ cài thông dụng → hỏi login shell. Trên
Windows không tìm gì vì tiến trình GUI ở đó vẫn thừa hưởng PATH của user.

Nghiệm thu:
- Chỉ nhận file thường có execute bit; thư mục tên `claude` bị bỏ qua
- Cài bằng installer chính (`~/.local/bin`) được ưu tiên trước cài qua npm
- Nhiều bản Node cùng tồn tại thì thứ tự thử cố định giữa các lần chạy
- Banner do file rc in ra không bị hiểu thành đường dẫn
- Hỏi login shell tối đa 5 giây rồi bỏ, không để rc file chậm làm treo poll
- Kết quả tìm được nhớ lại, chỉ tìm lại khi lệnh biến mất hoặc user đổi cấu hình
- Đo trên máy này, chạy test binary với PATH của launchd
  (`/usr/bin:/bin:/usr/sbin:/sbin`): lớp PATH thừa hưởng trả rỗng, lớp chỗ cài
  thông dụng trả `~/.local/bin/claude`
- Đo lớp login shell bằng HOME giả có `.zshrc` tự thêm PATH: tìm ra đúng file,
  bỏ đúng banner

### CUW-024 · Chạy lệnh trong thư mục riêng · done

Phụ thuộc: CUW-020, CUW-060

App không set `current_dir` khi sinh tiến trình, nên `claude` thừa hưởng cwd của
app. App mở từ Finder thì cwd là `/`, mà CLI coi cwd là project đang mở — nó làm
việc từ `/`, đi vào `Desktop`, `Music`, `Documents`, `Downloads`, `Pictures`.
macOS quy trách nhiệm truy cập của process con cho app bundle, nên người dùng bị
hỏi xin quyền từng thư mục một, dưới tên "Claude Usage". App không khai xin quyền
nào: Info.plist không có `NS*UsageDescription`.

Sửa: chạy lệnh trong `<thư mục cấu hình>/usage-cwd`, tạo lúc cần, để rỗng — CLI
không có gì để đi vào.

Nghiệm thu:
- Đo được trước khi sửa: 5 session file trong `~/.claude/projects/-` (`-` là slug
  của cwd `/`), tất cả mang `"cwd":"/"`, cách nhau ~3 phút đúng nhịp poll 180s
- Không file nào chứa `"usage"`, nên số tiền app báo không bị ảnh hưởng
- Áp cho cả Windows: ở đó cwd là thư mục chứa exe, cũng không phải project nào

Còn tồn: mỗi lần poll CLI vẫn ghi một session file ~2.4 KB vào
`~/.claude/projects/`. Nhịp mặc định 180s ra khoảng 480 file mỗi ngày, và chúng
nằm đúng trong thư mục mà scanner của app đang duyệt. Chưa xử lý.

---

## E4 — Dự đoán

### CUW-030 · Dự đoán cạn hạn mức · done

Phụ thuộc: CUW-021

Công thức trong `docs/architecture.md`, áp cho cả cửa sổ session và cửa sổ tuần.

Nghiệm thu:
- Trôi chưa tới 10 phút thì không hiện dự đoán
- Phần trăm đã dùng bằng 0 thì không hiện
- Dùng 50% sau 2.5 tiếng của cửa sổ 5 tiếng thì dự kiến chạm 100% đúng lúc reset
- Dùng 80% sau 2 tiếng thì ra thời điểm cạn sớm hơn reset, tính đúng tới phút

---

## E5 — Giao diện widget

### CUW-040 · Cửa sổ nổi · done

Phụ thuộc: CUW-002

Không viền, trong suốt, luôn nằm trên, kéo bằng nền, nhớ vị trí.

Nghiệm thu:
- Kéo được, vị trí còn nguyên sau khi mở lại
- Mở ngoài vùng màn hình thì tự kéo về trong màn hình
- Không cướp tiêu điểm khi cập nhật số

### CUW-041 · Thẻ hạn mức · done

Phụ thuộc: CUW-040, CUW-030

Phần trăm session, thanh ngang, đếm ngược tới reset, dòng dự đoán, hai thẻ tuần.

Nghiệm thu:
- Đếm ngược tự chạy mỗi giây, không chờ lần đọc `/usage` sau
- Màu đổi đúng ở mốc 50% và 80%
- Tắt `/usage` thì các thẻ này biến mất hẳn, không để lại ô trống

### CUW-042 · Thẻ tiền và biểu đồ · done

Phụ thuộc: CUW-040, CUW-014

Tiền hôm nay, so với trung bình 7 ngày, biểu đồ cột 14 ngày.

Nghiệm thu:
- Có chữ **theo giá API** kèm số tiền
- Biểu đồ vẽ bằng CSS hoặc SVG thuần, không nạp thư viện
- Ngày không có dữ liệu vẽ cột rỗng, không bỏ trống chỗ

### CUW-043 · Các thẻ còn lại · done

Phụ thuộc: CUW-042

Tỉ lệ model, cache hit rate, project tốn nhất, phiên đang chạy.

### CUW-044 · Hiện trạng thái bất thường · done

Phụ thuộc: CUW-041, CUW-022

Ba trạng thái trong `docs/ui-spec.md`: không đọc được, số đã cũ, giá ước lượng.

Nghiệm thu:
- Không bao giờ hiện số cũ mà trông như số mới
- Có model ngoài bảng giá thì tiền hiện kèm dấu ngã ở trước

### CUW-045 · Kéo cửa sổ từ mọi chỗ · done

Phụ thuộc: CUW-040

`data-tauri-drag-region` chỉ áp cho đúng phần tử mang thuộc tính, không áp cho
phần tử con — mà widget toàn thẻ con nên gần như không còn chỗ nào kéo được.
Thay bằng bắt `mousedown` rồi gọi `startDragging()`, trừ nút bấm và ô nhập.

Cần thêm quyền `core:window:allow-start-dragging`; `core:default` không có sẵn.

Nghiệm thu:
- Kéo được từ chỗ trống, từ chữ, từ thanh phần trăm
- Nút thử lại vẫn bấm được, không biến thành tay kéo
- Đã kiểm bằng chuột giả lập: cửa sổ đi theo. Tỉ lệ 1:1 thì không đo được vì
  Windows áp gia tốc con trỏ lên chuột giả lập, và việc kéo do hệ điều hành
  làm nên trong code không có chỗ nào nhân chia được

---

## E6 — Khay và taskbar

### CUW-050 · Icon khay và menu · done

Phụ thuộc: CUW-002

Nghiệm thu:
- Nhấn trái hiện hoặc ẩn cửa sổ nổi
- Nhấn phải ra menu: Mở cấu hình, Đọc lại ngay, Thoát
- Đóng cửa sổ không thoát app

### CUW-051 · Vẽ số vào icon khay trên Windows · done

Phụ thuộc: CUW-050

`set_title` không chạy trên Windows nên phải sinh ảnh 32x32 mỗi lần cập nhật.

Nghiệm thu:
- Đọc được ở cả nền sáng và nền tối của thanh tác vụ
- Đổi màu theo mức 50% và 80%
- Chỉ vẽ lại khi số đổi, không vẽ mỗi 3 giây

### CUW-054 · Hai số trong một icon khay · done

Phụ thuộc: CUW-051

Thêm lựa chọn `Cả hai, xếp chồng`: session ở trên, tuần ở dưới, mỗi dòng màu
theo mức của chính nó.

Xếp chồng chứ không đặt cạnh nhau vì icon khay là hình vuông. Đặt cạnh nhau
phải chia đôi bề ngang, tức chia đôi cỡ chữ; xếp chồng chỉ chia đôi chiều cao,
font 5x7 còn chịu được. Kết quả mỗi dòng cao 14px trong 32px, cách nhau 2px.

Nghiệm thu:
- Thiếu một trong hai cửa sổ thì không hiện gì, vì hiện một số lẻ sẽ bị đọc
  nhầm thành số kia
- Bốn lựa chọn cũ vẫn giữ nguyên một dòng

### CUW-052 · Chữ trên menu bar macOS · doing

Phụ thuộc: CUW-050

`set_title` với số chính. Đã kiểm trên máy macOS: chữ hiện đúng.

Chạy thật thì lộ ra: `update()` vừa vẽ số vào icon vừa gọi `set_title`, nên trên
macOS số hiện **hai lần** cạnh nhau — một lần theo màu mức (xanh/vàng/đỏ), một
lần theo màu chữ của menu bar. Bitmap chỉ tồn tại vì Windows bỏ qua `set_title`.
Đã sửa: `tray.rs` không vẽ bitmap trên macOS nữa, số chỉ nằm ở `set_title`.

Phần "icon đặt chế độ template" **không làm được với icon hiện tại**, và chưa
làm. Đo `src-tauri/icons/32x32.png`: 74% pixel đục hoàn toàn, 16% trong suốt,
hình alpha là một khối vuông bo góc đặc. macOS vẽ template theo alpha, nên bật
lên sẽ ra một ô đen đặc. Muốn có icon đơn sắc cho menu bar thì phải vẽ asset
riêng, dạng nét trên nền trong suốt.

Đã chốt: trên macOS bỏ icon, menu bar chỉ còn con số. `set_icon(None)` gọi trong
`update()` chứ không phải bỏ icon ngay ở `build()` — `label()` trả `None` khi
chưa có số, `update()` thoát sớm, nên bỏ từ `build()` sẽ để lại một mục rộng
bằng 0 không bấm được, đúng lúc người dùng cần mở cấu hình nhất.

Kéo theo: giới hạn 3 ký tự (`MAX_GLYPHS`) là ràng buộc của bitmap 32 pixel, đem
áp cho text menu bar thì cắt `12.3M` thành `12.`. Đã chuyển giới hạn đó về chỗ
vẽ bitmap; `label()` trả nguyên con số.

Đánh đổi đã biết: `set_title` là text trần nên màu mức (xanh/vàng/đỏ) không
truyền được lên menu bar. Màu mức giờ chỉ còn ở widget và badge trên Dock.

### CUW-053 · Chỉ báo trên taskbar và Dock · done

Phụ thuộc: CUW-050

`set_progress_bar` theo phần trăm. Windows thêm `set_overlay_icon`, macOS thêm
`set_badge_count`.

Nghiệm thu:
- Giấu cửa sổ khỏi taskbar thì tắt luôn phần này, không gọi API vô ích

---

## E7 — Cấu hình

### CUW-060 · Lưu và đọc cấu hình · done

Phụ thuộc: CUW-002

Lưu vào thư mục cấu hình của hệ điều hành, dạng JSON.

Nghiệm thu:
- Thiếu file thì dùng mặc định, không lỗi
- File hỏng thì dùng mặc định và ghi log, không xóa mất file cũ
- Trường lạ được giữ nguyên khi ghi lại

### CUW-061 · Cửa sổ cấu hình · done

Phụ thuộc: CUW-060

Toàn bộ mục trong `docs/ui-spec.md`.

### CUW-062 · Áp cấu hình ngay · done

Phụ thuộc: CUW-061

Nghiệm thu:
- Đổi nhịp đọc có tác dụng ngay, không cần mở lại app
- Tắt `/usage` thì dừng hẳn tiến trình định kỳ
- Đổi số chính ở khay thì icon đổi trong vòng một chu kỳ

### CUW-065 · Chọn tiếng Việt hay tiếng Anh · done

Phụ thuộc: CUW-061

Cấu hình có mục chọn ngôn ngữ, mặc định tiếng Việt.

Phải sửa kiến trúc trước: câu lỗi vốn viết cứng tiếng Việt trong Rust rồi đẩy
xuống giao diện. Nay backend trả **mã lỗi** (`not-logged-in`, `report-changed`,
`command-not-found`, `timed-out`, `command-failed`, `unreadable-output`), giao
diện mới chọn câu chữ. Phần chữ do hệ điều hành vẽ — menu khay, tooltip, thông
báo, thanh tiêu đề — thì Rust giữ bảng riêng trong `i18n.rs`.

Định dạng đi theo ngôn ngữ chứ không theo locale máy: tiếng Việt đọc đồng hồ
24 giờ và viết `4 ngày 12h`, tiếng Anh 12 giờ và `4d 12h`. Locale máy thường
là tiếng Anh ngay cả với người muốn giao diện tiếng Việt.

Nghiệm thu:
- Đổi ngôn ngữ áp ngay, không cần mở lại app
- Không còn chuỗi tiếng Việt viết cứng trong Rust ngoài `i18n.rs`
- Phần chi tiết lỗi do lệnh `claude` in ra thì giữ nguyên, không dịch

### CUW-066 · Rà lại câu chữ tiếng Việt · done

Phụ thuộc: CUW-065

Bỏ từ ẩn dụ, thay bằng cụm nói thẳng việc xảy ra:

| Trước | Sau |
|---|---|
| `cạn lúc 1:20 PM` | `hết hạn mức lúc 13:20` |
| `nhịp này → dùng 68% khi reset` | `theo nhịp hiện tại: 68% khi reset` |
| `theo giá API` | `quy đổi theo giá API` |
| `Session` | `Phiên 5 giờ` — kèm độ dài cửa sổ vì đó là thứ quyết định con số |
| `Tuần (chung)` | `Tuần · mọi model` |
| `Cache hit` | `Tỉ lệ đọc từ cache` |
| `số lúc 14:22` | `đọc lúc 14:22` |
| `gấp 2.0 lần trung bình 7 ngày` | `gấp 2.0 lần mức trung bình 7 ngày` |

Giữ nguyên từ tiếng Anh ở chỗ dịch ra sẽ khó nhận hơn: cache, model, reset,
token, project, output.

---

---

## E8 — Cảnh báo

### CUW-070 · Thông báo vượt mức · done

Phụ thuộc: CUW-030, CUW-060

Nghiệm thu:
- Mỗi mức chỉ báo một lần cho tới khi cửa sổ reset
- Tắt trong cấu hình thì không báo
- Mở app lúc đã vượt mức sẵn thì không báo dồn

---

## E9 — Đóng gói

### CUW-080 · Bộ icon app · done

Phụ thuộc: CUW-001

### CUW-081 · Bản cài Windows · done

Phụ thuộc: CUW-051, CUW-062

Đo được: installer **2.0 MB**, exe bên trong 8.8 MB (NSIS nén LZMA).

Nằm ở `src-tauri/target/release/bundle/nsis/Claude Usage_0.1.0_x64-setup.exe`.

Còn lại cần người kiểm: cài lên máy chưa có Rust.

Cách dựng sau proxy chặn: xem `README.md`, mục "Build khi proxy chặn GitHub".


### CUW-082 · Bản cài macOS · doing

Phụ thuộc: CUW-052, CUW-062

Hết chặn: đã có máy macOS (macOS 15, ARM). `npm run tauri build` chạy được,
mất 2 phút 7 giây, ra `bundle/dmg/Claude Usage_0.1.0_aarch64.dmg` 2.7 MB, binary
bên trong 6.2 MB Mach-O arm64.

Hai điều khác Windows, đã đo:
- Target `nsis` trong `tauri.conf.json` bị bỏ qua im lặng khi build trên macOS,
  không báo lỗi. Muốn có installer Windows thì phải build trên Windows.
- `bundle/macos/` rỗng sau khi build xong: Tauri copy `.app` vào `.dmg` rồi xóa
  (log in dòng `Cleaning ... Claude Usage.app`). Trên macOS chỉ có `.dmg` để đối
  chiếu thời điểm build.
- Lệnh kiểm thời điểm trong `CLAUDE.md` dùng `ls --time-style` là flag của GNU
  ls, BSD ls trên macOS không có. Dùng
  `stat -f '%Sm %z bytes %N' -t '%H:%M:%S' <file>`.

Chạy thật thì lộ ra một lỗi, đã sửa ở CUW-023: app đóng gói không tìm được lệnh
`claude` vì không nhận PATH của shell.

Còn lại cần người mở app lên kiểm bằng mắt, không tự động được:
- `set_title` hiện số trên menu bar (`tray.rs`)
- `set_badge_count` hiện số trên icon Dock (`window.rs`)
- Mục "giấu khỏi taskbar" phải biến mất khỏi cửa sổ cấu hình, vì
  `set_skip_taskbar` không chạy trên macOS
- Đường lưu cấu hình `~/Library/Application Support/claude-usage-widget/`

Bốn mục này chưa kiểm được từ phiên làm việc dòng lệnh: mở app GUI bằng `open`
từ đây trả về `_LSOpenURLsWithCompletionHandler() failed with error -10669`,
tức phiên shell này không nói chuyện được với LaunchServices của user.

### CUW-083 · Mở cùng hệ điều hành · done

Phụ thuộc: CUW-060
