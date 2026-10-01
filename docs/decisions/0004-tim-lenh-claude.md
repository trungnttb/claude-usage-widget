# 0004 — Tìm lệnh `claude` mà không dựa vào PATH thừa hưởng

Ngày: 2026-09-10 · Trạng thái: đã chốt

## Bối cảnh

Code ban đầu gọi `Command::new("claude")`, tức phó thác việc tìm file thực thi
cho PATH mà tiến trình thừa hưởng. Trên Windows cách này chạy được, nên lỗi nằm
im tới khi có máy macOS.

App đóng gói trên macOS do launchd mở, không do shell mở, nên nó nhận PATH của
launchd chứ không phải PATH trong terminal của user.

Đo trên máy đang phát triển:

- `launchctl getenv PATH` trả rỗng, tức không có gì ghi đè, app chỉ có bộ PATH
  tối thiểu của launchd.
- `claude` nằm ở `~/.local/bin/claude` (symlink tới
  `~/.local/share/claude/versions/<version>`) — đây là chỗ installer chính đặt.
- Chạy `command -v claude` với PATH `/usr/bin:/bin:/usr/sbin:/sbin` thì không
  thấy; thêm `~/.local/bin` vào thì thấy.

Kết quả với người dùng: widget hiện lỗi `command-not-found`, phần hạn mức trống,
dù `claude` chạy bình thường trong terminal. Phần tiền quy đổi từ transcript vẫn
đúng vì nó đọc file, không gọi lệnh.

## Các cách đã cân nhắc

1. **Dò danh sách chỗ cài cố định.** Rẻ, tất định, test được bằng thư mục tạm.
   Không tự tìm được bản cài qua npm dưới nvm, vì tên thư mục chứa số phiên bản
   Node.
2. **Hỏi login shell** (`$SHELL -lic 'command -v claude'`). Bắt được mọi cách
   cài, kể cả chỗ user tự đặt. Đổi lại: chạy file rc của user nên có thể chậm
   hoặc treo, và tốn khoảng một lần khởi động shell.
3. **Để user tự điền đường dẫn.** Luôn đúng, ít code nhất, nhưng trải nghiệm
   ngay sau khi cài là một thông báo lỗi.

## Quyết định

Xếp lớp cả ba, dừng ở lớp đầu tiên có kết quả:

1. `claudePath` trong cấu hình, nếu user có điền.
2. PATH thừa hưởng — vẫn là đường đúng khi app mở từ terminal, và là đường duy
   nhất dùng trên Windows.
3. Các chỗ installer ghi vào: `~/.local/bin`, `~/.claude/local`, `~/.bun/bin`,
   `~/.nvm/versions/node/*/bin`, `/opt/homebrew/bin`, `/usr/local/bin`.
4. Hỏi login shell, tối đa 5 giây.

Trên Windows bỏ hết lớp tìm kiếm, giữ nguyên tên lệnh trần: tiến trình GUI ở đó
vẫn thừa hưởng PATH của user, và cách cũ đã kiểm được là chạy (CUW-081).

Ràng buộc kèm theo:

- Chỉ nhận file thường có execute bit. Thư mục cũng có bit đó, nên một thư mục
  tên `claude` nằm trên PATH sẽ bị đem đi spawn rồi lỗi.
- Nhiều bản Node cùng tồn tại thì sắp theo tên để thứ tự thử cố định, không phụ
  thuộc thứ tự filesystem trả về.
- Output của login shell phải lọc, chỉ lấy dòng bắt đầu bằng `/`: file rc được
  quyền in banner của nó.
- Lớp login shell phải có giới hạn thời gian riêng. Nó chạy code của user; không
  giới hạn thì một rc file chờ input sẽ làm treo luôn lần poll đang cần nó.
- Kết quả tìm được nhớ lại trong `LimitsSource`, vì lớp cuối tốn một lần khởi
  động shell. Tìm lại khi lệnh biến mất (bản cài mới, hoặc gỡ đi) hoặc khi user
  đổi `claudePath`.

## Cái giá

Bốn lớp thì nhiều hơn một lời gọi `Command::new`, và lớp cuối chạy file rc của
user — chấp nhận được vì nó chỉ chạy khi ba lớp trên đã trượt, tức là máy có bản
cài mà app không biết trước.

Không lớp nào thay được ô `claudePath`: máy nào cài vào chỗ mà cả PATH lẫn login
shell đều không nhắc tới thì vẫn phải có đường cho user tự chỉ.
