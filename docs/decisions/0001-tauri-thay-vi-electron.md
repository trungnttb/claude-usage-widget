# 0001 — Dùng Tauri v2 thay vì Electron

Ngày: 2026-09-10 · Trạng thái: đã chốt

## Bối cảnh

App chạy nền suốt ngày làm việc và chỉ hiện vài con số. Ban đầu định dùng
Electron vì máy đã có sẵn Node 24 và không phải cài gì thêm.

Nghi vấn cần kiểm: có phải Electron chậm vì phải parse 100 MB JSONL không?

## Đo được gì

Parse toàn bộ transcript bằng Node:

```
122 file · 98.4 MB · 556 ms · 177 MB/s
```

Chỉ tốn 556 ms, và chỉ tốn một lần lúc mở app vì sau đó đọc theo kiểu ghi thêm.
Đổi sang Rust tiết kiệm được khoảng 400 ms, một lần. **Không phải lý do để đổi.**

Điểm thật sự khác nhau là bộ nhớ lúc app ngồi im:

| | Electron | Tauri v2 |
|---|---|---|
| RAM lúc rảnh | 150–250 MB | 40–80 MB |
| Bản cài | 80–150 MB | 3–8 MB |
| Cần cài thêm | không | Rust toolchain |
| Code giao diện | HTML/CSS/JS | HTML/CSS/JS (giống hệt) |

## Quyết định

Dùng Tauri v2.

Lý do quyết định không phải tốc độ mà là bộ nhớ: app bật 8–10 tiếng mỗi ngày để
liếc vài con số, chiếm 200 MB cho việc đó là không tương xứng. Phần code giao
diện không đổi vì Tauri vẫn dùng HTML/CSS/JS, nên chi phí đổi chỉ nằm ở lớp
backend.

Máy đã có WebView2 152.0.4191.66 nên người dùng cuối trên Windows 11 không cần
cài runtime. Đây cũng là lý do bản cài chỉ vài MB: Tauri dùng webview của hệ
điều hành thay vì đóng gói cả Chromium.

## Đánh đổi phải chịu

- Cần cài Rust toolchain (~1 GB, một lần) mới build được.
- Vòng lặp sửa-thử chậm hơn Electron vì phải biên dịch.
- Trên Linux, webview là WebKitGTK nên hành vi khác Chromium. Chưa hỗ trợ Linux,
  ghi lại để nhớ.

---

## Bổ sung 2026-09-10 — đo bản release xong, con số RAM ở trên là SAI

Bảng phía trên là ước lượng lấy từ tài liệu, không phải đo. Sau khi build xong
bản release và đo thật, phần dự đoán RAM không đúng.

Đo trên bản release, app chạy nền 15 giây, không thao tác gì:

| Tiến trình | WorkingSet | Private |
|---|---|---|
| `claude-usage-widget.exe` | 27.4 MB | 8.5 MB |
| 6 tiến trình `msedgewebview2` | 339.3 MB | 172.3 MB |
| **Tổng** | **366.7 MB** | **180.8 MB** |

Con số đáng tin là **Private ≈ 181 MB** (WorkingSet đếm trùng các trang bộ nhớ
dùng chung giữa các tiến trình).

**181 MB nằm đúng trong khoảng mình đã gán cho Electron (150–250 MB).** Lý do:
WebView2 hiện đại là Chromium đa tiến trình đầy đủ — nó sinh 6 tiến trình con,
không nhẹ hơn Electron bao nhiêu. Cái Tauri tiết kiệm là **không đóng gói** thêm
một bản Chromium nữa, chứ không phải không chạy Chromium.

### Vậy quyết định có sai không

Không sai, nhưng **lý do nêu ra thì sai**. Những gì đo được vẫn đúng:

| Tiêu chí | Dự đoán | Đo thật |
|---|---|---|
| Kích thước binary | 3–8 MB | **8.8 MB** (Electron: 80–150 MB) |
| Thời gian quét 98 MB JSONL | — | **207 ms** |
| RAM lúc rảnh | 40–80 MB | **181 MB** — sai |

Còn một chi tiết làm bức tranh khác đi: lúc đo, máy **đã có sẵn 32 tiến trình
`msedgewebview2`** của các app khác. WebView2 là runtime dùng chung của hệ điều
hành, nên trên máy đã có app WebView2 khác chạy thì phần tăng thêm thấp hơn con
số 181 MB này. Electron thì mỗi app cõng một bản Chromium riêng, không chia sẻ
được với ai.

### Rút ra

Nếu chỉ tiêu duy nhất là RAM lúc rảnh thì hai bên ngang nhau, và Electron còn
dựng nhanh hơn. Giữ Tauri vì bản cài nhỏ hơn hơn 10 lần, không cần Node runtime,
và dùng chung webview với phần còn lại của hệ điều hành — không phải vì nó nhẹ
RAM như đã viết ban đầu.

Muốn thật sự xuống dưới 50 MB thì phải bỏ webview, tức viết native từng OS, giá
là hai codebase. Chưa làm; ghi lại để sau này cân nhắc lại có căn cứ.
