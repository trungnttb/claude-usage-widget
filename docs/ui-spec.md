# Giao diện

## Cửa sổ nổi

Không viền, bo góc, luôn nằm trên, kéo được bằng cách giữ chuột vào nền. Rộng
mặc định 260 px, cao co theo số thẻ đang bật. Nhớ vị trí giữa các lần mở.

```
┌──────────────────────────────────┐
│  Session          41%            │   ← dòng chính, chọn được hiện gì
│  ████████░░░░░░░░  còn 1h52m     │
│  nhịp này → dùng 68% khi reset   │   ← dự đoán
├──────────────────────────────────┤
│  Tuần (chung)     23%  ███░░░░   │
│  Tuần (Fable)     22%  ███░░░░   │
├──────────────────────────────────┤
│  Hôm nay      $85.31             │
│  gấp 1.4 lần trung bình 7 ngày   │
│  ▁▂▅▃█▂▁▄▆▃▂▅█▃                  │   ← 14 ngày
├──────────────────────────────────┤
│  opus-5   68%   fable-5-1  31%   │
│  cache hit 99.1%                 │
│  jb-e2e  $41.20                  │
└──────────────────────────────────┘
```

Màu theo mức đã dùng: dưới 50% xanh, 50–80% vàng, trên 80% đỏ. Áp cho cả thanh
ngang, icon khay, và thanh trên nút taskbar.

## Khay hệ thống và taskbar

Ràng buộc nền tảng đã xác minh, chi tiết trong `AGENTS.md`:

| | Windows | macOS |
|---|---|---|
| Khay / menu bar | `set_title` **không chạy** → vẽ số vào icon 32×32 | `set_title("41%")` hiện chữ thẳng, **không icon** |
| Tooltip khi rê chuột | có | có |
| Nút taskbar / Dock | `set_progress_bar` vẽ thanh theo % · `set_overlay_icon` gắn badge | `set_progress_bar` trên Dock · `set_badge_count` hiện số |

Hai cách này loại nhau, không cộng vào nhau. Làm cả hai trên macOS thì con số
hiện hai lần cạnh nhau — một lần theo màu mức, một lần theo màu chữ menu bar.
Nên trên macOS `update()` gọi `set_icon(None)`: chỉ còn con số.

Kéo theo hai điều:

- Giới hạn 3 ký tự là ràng buộc của bitmap 32 pixel, không áp cho text. Áp cho
  text thì `12.3M` bị cắt thành `12.`.
- `set_title` là text trần nên **không mang được màu mức**. Trên macOS màu mức
  chỉ còn ở widget và badge trên Dock.

Nhấn trái vào icon khay: hiện/ẩn cửa sổ nổi. Nhấn phải: menu gồm Mở cấu hình,
Đọc lại ngay, Thoát.

## Thẻ bật tắt được

Toàn bộ danh sách, kèm trạng thái mặc định:

| Thẻ | Mặc định | Nguồn |
|---|---|---|
| Phần trăm session + đếm ngược tới reset | bật | `/usage` |
| Dự đoán cạn hạn mức | bật | tính ra |
| Phần trăm tuần chung | bật | `/usage` |
| Phần trăm tuần theo model | bật | `/usage` |
| Tiền hôm nay + so với trung bình 7 ngày | bật | JSONL |
| Biểu đồ cột 14 ngày | bật | JSONL |
| Tỉ lệ model hôm nay | bật | JSONL |
| Cache hit rate | tắt | JSONL |
| Project tốn nhất hôm nay | tắt | JSONL |
| Phiên đang chạy | tắt | JSONL |

## Cửa sổ cấu hình

**Hiển thị**
- Số chính ở khay: `phần trăm session` (mặc định) / `phần trăm tuần` /
  `tiền hôm nay` / `token hôm nay`
- Bật tắt từng thẻ trong bảng trên
- Đơn vị tiền: USD, hoặc VND kèm ô nhập tỉ giá

**Nguồn**
- Bật tắt `/usage`. Tắt thì mọi thẻ phụ thuộc nó biến mất, không hiện ô trống
- Nhịp đọc JSONL: mặc định 3 giây
- Nhịp chạy `/usage`: mặc định 3 phút, tối thiểu 60 giây

**Cảnh báo**
- Bật tắt thông báo hệ thống
- Mức cảnh báo session: mặc định 80%
- Mức cảnh báo tuần: mặc định 85%
- Chỉ báo một lần mỗi khi vượt mức, không lặp lại tới khi cửa sổ reset

**Cửa sổ**
- Luôn nằm trên: bật
- Độ trong suốt nền: 0–100%
- Giấu khỏi taskbar: tắt trên Windows, không có mục này trên macOS
- Mở cùng hệ điều hành: tắt

## Trạng thái phải hiện rõ

Ba trạng thái dưới đây dễ bị bỏ sót và làm người dùng tin nhầm số:

1. **Chưa đọc được `/usage`** — hiện `hạn mức: không đọc được` kèm nút thử lại,
   không hiện số cũ như thể còn mới.
2. **Số `/usage` đã cũ** — khi lần đọc gần nhất quá hai chu kỳ, hiện thêm
   `số lúc HH:MM` bên cạnh.
3. **Có model ngoài bảng giá** — hiện dấu `~` trước số tiền và giải thích khi rê
   chuột, vì lúc đó tiền là ước lượng theo mức Opus.

## Nguyên tắc chữ nghĩa

Số tiền luôn kèm ngữ cảnh **"theo giá API"**. Đây không phải tiền bị trừ khỏi
tài khoản subscription. Nhầm chỗ này là nhầm nghiêm trọng nhất mà app có thể gây ra.
