# 0002 — Chỉ hai nguồn dữ liệu, bỏ Admin API

Ngày: 2026-09-10 · Trạng thái: đã chốt

## Bối cảnh

Ban đầu định ba nguồn: transcript local, `claude -p "/usage"`, và Usage & Cost
Admin API để đối chiếu số tiền Anthropic thật sự tính.

## Vì sao bỏ Admin API

Đọc tài liệu chính thức thì thấy ba rào cùng lúc:

1. **"The Admin API is unavailable for individual accounts."** Phải là tổ chức
   trên Claude Console và phải có admin key. Phần lớn người dùng app này không có.
2. **Không realtime.** `/v1/organizations/usage_report/claude_code` trễ tới 1 giờ
   và chỉ gom theo ngày. Chậm hơn hẳn transcript local vốn cập nhật ngay khi
   Claude Code ghi file.
3. **Không thêm thông tin quyết định được gì.** Nó cho biết số tiền chính xác
   hơn, nhưng với tài khoản subscription thì số tiền không phải thứ bị trừ — hạn
   mức mới là. Mà hạn mức thì `/usage` đã cho rồi.

Thêm một nguồn nghĩa là thêm màn hình nhập key, thêm chỗ cất key an toàn, thêm
đường hỏng phải xử lý. Đổi lại gần như không thêm giá trị.

## Quyết định

Hai nguồn:

| Nguồn | Cho gì | Nhịp | Bật mặc định |
|---|---|---|---|
| transcript JSONL | token, tiền quy đổi theo giá API, chia theo project và model | 3 giây | có, không tắt được |
| `claude -p "/usage"` | phần trăm hạn mức session và tuần | 3 phút | có, tắt được |

App không gọi mạng. Không có chỗ nhập key ở đâu cả.

## Hệ quả

Con số tiền app hiện là **"nếu tính theo giá API"**, không phải tiền bị trừ khỏi
tài khoản. Giao diện phải nói rõ điều này, nếu không người dùng sẽ hiểu nhầm là
hóa đơn.
