# Kiến trúc

## Nhìn tổng thể

```
┌─ src-tauri (Rust) ──────────────────────────────────────┐
│                                                          │
│  transcript.rs ──┐                                       │
│  (đọc JSONL,     │                                       │
│   3 giây/lần)    │                                       │
│                  ├──► snapshot.rs ──► tray.rs            │
│  usage_cli.rs ───┘    (gộp thành      (Windows: vẽ số    │
│  (chạy claude -p       một struct)     vào icon · macOS: │
│   /usage, 3 phút/lần)      │           số là text)       │
│                            │ emit "snapshot"             │
│  settings.rs ──────────────┤                             │
│  (đọc/ghi cấu hình)        │                             │
└────────────────────────────┼─────────────────────────────┘
                             ▼
┌─ src (HTML/CSS/JS) ─────────────────────────────────────┐
│  widget    cửa sổ nổi, không viền, luôn nằm trên         │
│  settings  cửa sổ cấu hình                               │
└──────────────────────────────────────────────────────────┘
```

Không framework, không bước build cho frontend. Ba file HTML, CSS thuần, JS
thuần. Lý do: giao diện chỉ hiện vài con số và một biểu đồ cột; thêm React vào
đây làm bản cài to lên mà không đổi được gì cho người dùng.

## Luồng dữ liệu

Backend là nơi duy nhất giữ trạng thái. Frontend không tự tính gì, chỉ vẽ lại
struct nhận được.

1. Hai bộ thu thập chạy độc lập theo nhịp riêng, mỗi bộ ghi kết quả vào phần
   của mình trong trạng thái chung.
2. Mỗi lần một bộ ghi xong, backend dựng lại `Snapshot` và phát sự kiện
   `snapshot` xuống frontend.
3. Frontend nhận `Snapshot`, vẽ lại. Không hỏi ngược, không tự gọi lệnh.

Hệ quả: hai nguồn hỏng độc lập. `/usage` không đọc được thì phần trăm hiện
trạng thái không rõ, còn số tiền từ transcript vẫn chạy bình thường.

## `Snapshot` — hợp đồng giữa Rust và giao diện

Đây là thứ duy nhất frontend biết. Mọi thay đổi ở đây phải sửa cả hai phía.

```rust
struct Snapshot {
    limits: Option<Limits>,        // None khi chưa đọc được /usage
    limits_read_at: Option<i64>,   // để hiện "số lúc HH:MM"
    limits_error: Option<String>,

    today: DayTotals,
    days: Vec<DayTotals>,          // 14 ngày gần nhất, cho biểu đồ cột
    avg_7d_cost: f64,

    by_model: Vec<ModelTotals>,    // hôm nay
    by_project: Vec<ProjectTotals>,// hôm nay, đã sắp giảm dần
    current_session: Option<SessionTotals>,
    cache_hit_rate: f64,           // cache_read / tổng input

    forecast: Option<Forecast>,    // dự đoán cạn hạn mức
    scanned_at: i64,
    has_estimated_pricing: bool,   // true khi gặp model ngoài bảng giá
}
```

## Dự đoán cạn hạn mức

Thứ hữu ích nhất trên giao diện, và là phần duy nhất backend tự suy ra thay vì
đọc thẳng.

Cách tính, cho cả cửa sổ session lẫn cửa sổ tuần:

```
đã_trôi   = bây_giờ - (thời_điểm_reset - độ_dài_cửa_sổ)
còn_lại   = thời_điểm_reset - bây_giờ
nhịp      = phần_trăm_đã_dùng / đã_trôi
dự_kiến   = phần_trăm_đã_dùng + nhịp * còn_lại
```

- `dự_kiến < 100` → hiện `nhịp này → dùng N% khi reset`
- `dự_kiến >= 100` → giải ra thời điểm chạm 100%, hiện `cạn lúc HH:MM · sớm hơn
  reset Xh Ym`

Điều kiện để không hiện số vô nghĩa: bỏ qua khi `đã_trôi` dưới 10 phút (nhịp lúc
đó nhiễu quá mạnh), và khi `phần_trăm_đã_dùng` bằng 0.

Độ dài cửa sổ session là 5 tiếng, cửa sổ tuần là 7 ngày. Cả hai suy ra từ thời
điểm reset mà `/usage` trả về, không hard-code mốc bắt đầu.

## Vì sao tách `pricing.rs` riêng

Bảng giá là thứ thay đổi từ bên ngoài, không phụ thuộc phần còn lại của app.
Tách riêng để: sửa giá không đụng logic đọc file, và test được bằng số cụ thể
mà không cần dựng file JSONL giả.

## Đọc thêm

- `docs/data-sources.md` — hai nguồn, kèm số đo thật
- `docs/ui-spec.md` — bố cục và các mục cấu hình
- `docs/decisions/` — vì sao chọn như vậy
