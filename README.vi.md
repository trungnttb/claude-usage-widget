# Claude Usage Widget

[English](README.md) · **Tiếng Việt**

Widget nổi và icon khay hệ thống hiển thị mức dùng Claude Code: còn bao nhiêu
phần trăm hạn mức, và lưu lượng token quy ra tiền theo giá API.

Chạy trên Windows và macOS. Không gọi mạng, mọi số đọc từ máy của bạn.

## Nó hiện gì

```
┌──────────────────────────────────┐
│  Session          41%            │
│  ████████░░░░░░░░  còn 1h52m     │
│  nhịp này → dùng 68% khi reset   │
├──────────────────────────────────┤
│  Tuần (chung)     23%  ███░░░░   │
│  Tuần (Fable)     22%  ███░░░░   │
├──────────────────────────────────┤
│  Hôm nay      $85.31             │
│  gấp 1.4 lần trung bình 7 ngày   │
│  ▁▂▅▃█▂▁▄▆▃▂▅█▃                  │
└──────────────────────────────────┘
```

Dòng hữu ích nhất là dòng thứ ba: nó không nói bạn đang ở đâu mà nói bạn sắp
gặp gì. Nếu nhịp dùng hiện tại sẽ làm cạn hạn mức trước lúc reset, nó ghi thẳng
thời điểm cạn.

## Số tiền nghĩa là gì

Là **số tiền nếu lưu lượng đó bị tính theo giá API**, không phải tiền bị trừ
khỏi tài khoản. Với gói subscription thì thứ bị tiêu là hạn mức chứ không phải
tiền, nên hai con số này khác nhau. App hiện cả hai và ghi rõ cái nào là cái nào.

## Lấy dữ liệu từ đâu

Hai nguồn, đều nằm sẵn trên máy:

| Nguồn | Cho gì | Nhịp đọc |
|---|---|---|
| `~/.claude/projects/**/*.jsonl` | token, tiền quy đổi, chia theo project và model | 3 giây |
| `claude -p "/usage"` | phần trăm hạn mức session và tuần | 3 phút |

Lệnh `/usage` không gọi model nên không tốn token — đo được `total_cost_usd: 0`
và `num_turns: 0` trong chính JSON nó trả về.

App **không** đọc `~/.claude/.credentials.json` và **không** gửi gì ra ngoài.
Lý do trong [ADR 0003](docs/decisions/0003-khong-doc-credentials.md).

App tự tìm file `claude`, không dựa vào `PATH` thừa hưởng — app mở từ Finder
trên macOS không nhận `PATH` của terminal. Nếu vẫn báo không tìm thấy thì chạy
`which claude` rồi dán đường dẫn vào ô **Đường dẫn tới lệnh `claude`** trong cửa
sổ cấu hình. Lý do và bốn lớp tìm kiếm: [ADR 0004](docs/decisions/0004-tim-lenh-claude.md).

## Cài để phát triển

Cần Rust toolchain:

```bash
winget install Rustlang.Rustup                                      # Windows
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh      # macOS
```

Windows 11 đã có sẵn WebView2 nên không cần cài thêm. macOS dùng WKWebView của
hệ thống, cũng không cần cài, nhưng cần Xcode Command Line Tools cho linker:

```bash
xcode-select --install     # bỏ qua nếu đã có
```

Sau đó:

```bash
npm install
npm run tauri dev
```

Đóng gói: `npm run tauri build`. Chạy trên macOS ra `.dmg`, chạy trên Windows ra
installer NSIS. Không cross-compile được, và target không thuộc nền tảng đang
chạy bị bỏ qua **không báo lỗi**.

## Build khi proxy chặn GitHub

`npm run tauri build` cần tải bộ NSIS về để đóng gói installer. Sau proxy chặn
`github.com` thì bước này hỏng với `CONNECT proxy failed: 407`, dù bản thân
`cargo` và `npm` vẫn chạy được.

Cách chữa: lấy hai file bằng đường khác rồi đặt sẵn vào chỗ Tauri tìm, nó sẽ
bỏ qua bước tải.

| File | Lấy ở đâu | SHA1 |
|---|---|---|
| `nsis-3.11.zip` | `github.com/tauri-apps/binary-releases/releases/download/nsis-3.11/` | `EF7FF767E5CBD9EDD22ADD3A32C9B8F4500BB10D` |
| `nsis_tauri_utils.dll` | `github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/` | `75197FEE3C6A814FE035788D1C34EAD39349B860` |

```bash
D="$LOCALAPPDATA/tauri/NSIS"
mkdir -p "$D" && unzip -q nsis-3.11.zip -d /tmp/nx && cp -r /tmp/nx/nsis-3.11/. "$D/"
mkdir -p "$D/Plugins/x86-unicode" && cp nsis_tauri_utils.dll "$D/Plugins/x86-unicode/"
```

Tauri kiểm đúng 13 đường dẫn dưới đây; **thiếu một cái là nó xoá cả thư mục rồi
tải lại từ đầu**, nên phải đủ hết trước khi chạy build:

```
makensis.exe                    Include/MUI2.nsh
Bin/makensis.exe                Include/FileFunc.nsh
Stubs/lzma-x86-unicode          Include/x64.nsh
Stubs/lzma_solid-x86-unicode    Include/nsDialogs.nsh
Plugins/x86-unicode/            Include/WinMessages.nsh
  nsis_tauri_utils.dll          Include/Win/COM.nsh
                                Include/Win/Propkey.nsh
                                Include/Win/RestartManager.nsh
```

Phiên bản phải khớp: Tauri CLI hiện tại đòi NSIS 3.11 và plugin v0.5.3, bản khác
sẽ bị từ chối theo SHA1.

## Kiểm tra trước khi commit

```bash
cargo fmt   --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test  --manifest-path src-tauri/Cargo.toml
```

## Tài liệu

| File | Nội dung |
|---|---|
| [docs/backlog.md](docs/backlog.md) | hàng đợi công việc, 35 mục, có tiêu chí nghiệm thu |
| [docs/data-sources.md](docs/data-sources.md) | hai nguồn dữ liệu kèm số đo thật |
| [docs/architecture.md](docs/architecture.md) | kiến trúc và luồng dữ liệu |
| [docs/ui-spec.md](docs/ui-spec.md) | bố cục và các mục cấu hình |
| [docs/decisions/](docs/decisions/) | vì sao chọn Tauri, vì sao hai nguồn, vì sao không đụng token |
| [AGENTS.md](AGENTS.md) | quy tắc cho AI agent làm việc trong repo |

## Số đo thật

Đo trên bản release:

| | Windows 11 | macOS 15 ARM |
|---|---|---|
| Bản cài | 2.0 MB (NSIS) | 2.7 MB (`.dmg`) |
| Kích thước binary | 8.8 MB | 6.2 MB (Mach-O arm64) |
| Build lại từ đầu | — | 2 phút 7 giây |
| RAM lúc rảnh | 181 MB private (app 8.5 MB + 6 tiến trình WebView2) | chưa đo |
| Quét 98 MB transcript | 207 ms | chưa đo |
| Một lần chạy `claude -p /usage` | ~2.9 giây, 0 token | ~2.9 giây, 0 token |

Con số RAM cao hơn hẳn dự đoán ban đầu và làm lung lay lý do chọn Tauri — xem
[ADR 0001](docs/decisions/0001-tauri-thay-vi-electron.md), phần bổ sung cuối file.

## Trạng thái

33/35 mục xong. Bản cài macOS đã dựng được, không còn chặn.

Hai mục còn lại đều đang chờ người mở app trên macOS xem bằng mắt, không phải
chờ code:

- **CUW-052** chữ trên menu bar. Đã sửa lỗi hiện số hai lần; còn lại là chốt
  hình icon đơn sắc, mà icon app hiện tại không dùng được cho chế độ template
  (alpha của nó là khối vuông đặc, bật lên sẽ ra một ô đen).
- **CUW-082** bản cài macOS. Còn phải kiểm badge trên icon Dock, mục "giấu khỏi
  taskbar" phải không xuất hiện trong cấu hình, và đường lưu cấu hình
  `~/Library/Application Support/claude-usage-widget/`.

Ba lỗi chỉ lộ ra khi thực sự chạy trên macOS, đều đã sửa: không tìm được lệnh
`claude` (CUW-023), số hiện hai lần trên menu bar (CUW-052), và app xin quyền
`Desktop`/`Music` vì tiến trình con thừa hưởng cwd `/` (CUW-024).

Chi tiết trong [docs/backlog.md](docs/backlog.md).

## Giấy phép

[MIT](LICENSE)
