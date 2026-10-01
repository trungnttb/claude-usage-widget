# 0003 — Không đọc `.credentials.json`

Ngày: 2026-09-10 · Trạng thái: đã chốt

## Bối cảnh

`~/.claude/.credentials.json` chứa `claudeAiOauth.accessToken`, kèm
`subscriptionType` và `rateLimitTier`. Về mặt kỹ thuật, app có thể đọc token đó
rồi gọi thẳng endpoint mà lệnh `/usage` trong Claude Code vẫn gọi, và lấy phần
trăm hạn mức nhanh hơn cách chạy CLI (2.9 giây).

## Vì sao không làm

1. **Token có toàn quyền tài khoản.** Không phải key chỉ đọc. Một app widget
   không có lý do gì để cầm nó.
2. **Endpoint không có tài liệu công khai.** Anthropic đổi lúc nào cũng được và
   không nợ ai thông báo. Xây app quanh nó là xây trên nền có thể mất bất cứ lúc nào.
3. **Đã có đường thay thế được hỗ trợ.** `claude -p "/usage"` cho đúng số đó,
   không tốn token, và là giao diện công khai nên ổn định hơn nhiều.

Chi phí phải trả cho lựa chọn này là 2.9 giây mỗi lần đọc thay vì một lời gọi
HTTP. Với nhịp 3 phút thì không đáng kể.

## Quyết định

App không mở `.credentials.json` trong bất kỳ trường hợp nào. Không gọi endpoint
nội bộ. Không tự gọi mạng.

Đây là điều kiện bắt buộc, ghi trong `AGENTS.md`. Task nào có vẻ cần vi phạm thì
dừng lại và hỏi người dùng, không tự quyết.
