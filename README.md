# C9 Social Assistant - Bóc tách bài viết & Bình luận Facebook

Hệ thống phần mềm thương mại trích xuất, bóc tách dữ liệu bài viết và bình luận Facebook phục vụ kinh doanh online, phát hiện số điện thoại và phân loại đơn hàng tự động.

## 🏗 Kiến trúc Hệ thống

### 1. Server Backend (Python FastAPI + PostgreSQL)
- **Framework**: FastAPI (Async/Await), SQLAlchemy 2.0.
- **Bảo mật Bản quyền Trực tuyến (Online-Only)**:
  - Sử dụng mật mã bất đối xứng **Ed25519** (`cryptography`).
  - Server nắm Private Key để ký session token tạm thời (chu kỳ 15 phút).
  - Client chỉ nhúng Public Key để verify tính toàn vẹn chữ ký số.
- **Quản lý HWID Thiết bị**:
  - Tự động băm SHA-256 từ CPU ID, Serial bo mạch chủ và Disk UUID.
  - Kiểm soát số lượng thiết bị (`max_devices`) trên mỗi bản quyền.
- **Cloud Sync**:
  - Nhận đồng bộ bài viết và bình luận từ desktop client (`/api/v1/sync/batch`).

### 2. Client Desktop App (Tauri v2 - Rust & React + TailwindCSS)
- **Framework**: Tauri v2, Rust native backend, React 18, TailwindCSS.
- **Chống dịch ngược & Dò chuỗi**:
  - Profile Release tối ưu kích thước và loại bỏ bảng ký hiệu: `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`.
  - Che giấu chuỗi nhạy cảm (API URLs, Ed25519 Public Key, Phone regex) trong nhị phân bằng macro `obfstring`.
- **Cơ chế Bản quyền Nghiêm ngặt (Strict Online-Only)**:
  - Không có chế độ Offline. Khi khởi động ứng dụng xác thực với server qua Ed25519.
  - Heartbeat worker chạy ngầm mỗi 5 phút gửi `/heartbeat`. Nếu server trả lỗi 403 hoặc mất mạng quá 2 chu kỳ (10 phút), app lập tức khóa giao diện.
- **Cào dữ liệu tốc độ cao (In-App Facebook GraphQL)**:
  - Gọi thẳng doc_id GraphQL nội bộ của Facebook với HTTP/2 và session cookies (`c_user`, `xs`, `datr`, `fr`, `fb_dtsg`).
  - Jitter Delay ngẫu nhiên từ 1.8s - 3.8s giữa các cursor phân trang.
  - Bóc tách số điện thoại Việt Nam tự động và phân loại ý định:
    - `[Chốt đơn]`: Khách có SĐT hoặc cú pháp đặt hàng, size, màu, địa chỉ.
    - `[Hỏi giá]`: Khách hỏi giá, inbox tư vấn.
    - `[Khiếu nại]`: Phản hồi sự cố hàng hóa, giao hàng.
    - `[Spam]`: Tin nhắn quảng cáo, link ngoài.
- **Lưu trữ Cục bộ SQLite WAL Mode**:
  - Kích hoạt `PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;`.
  - Thực thi ghi dữ liệu theo từng Batch Transaction (500 items/commit) giữ UI luôn mượt mà.
- **Quét gia tăng (Incremental Crawling)**:
  - Tự động so sánh mốc thời gian và cursor cũ, dừng ngay khi chạm dữ liệu đã có để tiết kiệm băng thông và bảo vệ tài khoản Facebook.

---

## 🚀 Hướng Dẫn Chạy Nhanh

### Bước 1: Khởi động Backend Server
```bash
# Sử dụng Python 3.12+
cd backend
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt

# Khởi tạo khóa Ed25519 và license mẫu (C9-PRO-2026-VIP)
python seed.py

# Khởi chạy server FastAPI
python -m uvicorn server.main:app --port 8000 --reload
```

### Bước 2: Khởi động Desktop Client
```bash
# Yêu cầu Node 24 (sử dụng nvm use 24) và Cargo/Rust
nvm use 24
cd pc_client_app
npm install

# Chạy giao diện Web Dev
npm run dev

# Hoặc chạy toàn diện với cửa sổ Desktop Tauri
npm run tauri dev
```

### Bước 3: Đăng nhập Bản quyền & Thử nghiệm
1. Mở ứng dụng, nhập License Key: `C9-PRO-2026-VIP`.
2. Máy chủ: `http://127.0.0.1:8000`.
3. Bấm **"Kích hoạt Bản quyền"** (Chữ ký Ed25519 sẽ được xác minh).
4. Nhấn nút **"Dữ liệu Mẫu"** để nạp ngay bài viết và danh sách bình luận test.
5. Xem bóc tách số điện thoại, lọc ý định, hoặc xuất file Excel/CSV.
