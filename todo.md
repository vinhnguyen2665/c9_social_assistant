Bạn là một Principal Software Architect và Security Engineer. Hãy thiết kế chi tiết kiến trúc và viết toàn bộ mã nguồn lõi cho một hệ thống phần mềm thương mại trích xuất, bóc tách dữ liệu bài viết/bình luận Facebook phục vụ kinh doanh online và sáng tạo nội dung.

Hệ thống bao gồm 2 phân hệ độc lập:
1. Server Backend (Python): Quản lý bản quyền Online, xác thực HWID thiết bị, cấp session token và nhận đồng bộ dữ liệu.
2. Client Desktop App (Tauri v2 - Rust & React): Ứng dụng desktop đa nền tảng (Windows/macOS), cào dữ liệu qua In-app WebView/GraphQL, quản lý danh sách post theo dõi, bóc tách số điện thoại/đơn hàng, lưu trữ SQLite cục bộ và bảo vệ bản quyền trực tuyến.

================================================================================
PHẦN 1: SERVER BACKEND (PYTHON - FASTAPI + POSTGRESQL)
================================================================================

1. Yêu cầu công nghệ:
- Framework: FastAPI (bất đồng bộ async/await).
- ORM/Database: SQLAlchemy 2.0 (Async) + PostgreSQL.
- Bảo mật License: Sử dụng mã hóa bất đối xứng Ed25519 (thư viện `cryptography`). Server nắm giữ Private Key để ký session token tạm thời, Client chỉ nắm Public Key để xác minh tính toàn vẹn.

2. CSDL Backend (PostgreSQL Schema):
- Bảng `licenses`:
  * `id` (UUID PK), `license_key` (VARCHAR UNIQUE, indexed), `client_name` (VARCHAR), `max_devices` (INT DEFAULT 1), `expires_at` (TIMESTAMPTZ), `is_active` (BOOLEAN DEFAULT TRUE), `created_at` (TIMESTAMPTZ).
- Bảng `activated_devices`:
  * `id` (UUID PK), `license_id` (UUID FK trỏ licenses.id), `hwid` (VARCHAR - SHA256 phần cứng), `device_name` (VARCHAR), `current_session_token` (VARCHAR), `last_heartbeat` (TIMESTAMPTZ), `is_revoked` (BOOLEAN DEFAULT FALSE).
- Bảng `cloud_posts` & `cloud_comments`: Nhận dữ liệu backup/đồng bộ từ client khi người dùng yêu cầu.

3. Các API Endpoints:
- `POST /api/v1/license/login`:
  * Nhận: `{ license_key, hwid, device_name }`
  * Logic: Kiểm tra key tồn tại, còn hạn (`expires_at > now`), đang kích hoạt. Kiểm tra số HWID đã đăng ký (`max_devices`). Nếu thiết bị hợp lệ: sinh `session_token` ngẫu nhiên có hạn 15 phút, cập nhật vào bảng `activated_devices`.
  * Trả về: `signed_payload` gồm chuỗi JSON `{hwid, session_token, exp, license_key}` được ký số bởi Ed25519 Private Key.
- `POST /api/v1/license/heartbeat`:
  * Nhận: `{ license_key, hwid, session_token }`
  * Logic: Kiểm tra `session_token` khớp trong DB, key chưa bị thu hồi/hết hạn. Cập nhật `last_heartbeat`.
  * Trả về: `signed_payload` gia hạn thêm 15 phút. Nếu vi phạm, trả mã HTTP 403 Forbidden.
- `POST /api/v1/sync/batch`: Nhận danh sách posts/comments từ client để bulk insert vào database cloud.

================================================================================
PHẦN 2: CLIENT DESKTOP APP (TAURI V2 - RUST & REACT)
================================================================================

1. Yêu cầu kiến trúc & Chống dịch ngược:
- Framework: Tauri v2 (Rust backend + React/TypeScript UI + TailwindCSS).
- Tránh dịch ngược:
  * File `Cargo.toml`: Cấu hình Release tối ưu: `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`. Biên dịch thẳng ra native machine code (ELF/PE).
  * Chuỗi nhạy cảm: Dùng macro `obfstr` mã hóa toàn bộ URL API, Public Key, regex bóc tách lúc compile để ngăn chặn lệnh trích xuất chuỗi thô (`strings`).
- Cơ chế Hardware ID: Tạo module Rust đọc và băm SHA256 tổ hợp từ CPU ID, Serial bo mạch chủ và UUID ổ cứng chính.

2. Quy tắc Bản quyền Online bắt buộc (Strict Online-Only):
- Ứng dụng KHÔNG có chế độ Offline. Không có mạng hoặc lỗi server -> Dừng ngay tại màn hình khóa bản quyền.
- Khi khởi động: Gửi `/login` lên server, dùng Ed25519 Public Key nhúng sẵn verify chữ ký số của server.
- Background Heartbeat: Worker chạy ngầm mỗi 5 phút gửi `/heartbeat`. Nếu server trả lỗi 403 hoặc mất mạng quá 2 chu kỳ (10 phút), app lập tức ngắt toàn bộ tiến trình cào, đóng kết nối SQLite và khóa giao diện.

3. Cơ chế cào: Tốc độ cao, Giống người, Tránh checkpoint (GraphQL/Session):
- KHÔNG dùng bot tự động cuộn chuột giao diện (UI scrolling) vì nặng máy và dễ dính cờ bot.
- Tích hợp In-app WebView để người dùng tự đăng nhập tài khoản Facebook (hỗ trợ 2FA).
- Rust trích xuất session cookies (`c_user`, `xs`, `datr`, `fr`) và `fb_dtsg`.
- Dùng `reqwest` (Rust) với HTTP/2, giả lập đầy đủ header của trình duyệt gốc (`sec-ch-ua`, `User-Agent`).
- Gọi thẳng các doc_id / GraphQL endpoint nội bộ của Facebook để phân trang comment và post.
- Jitter Delay: Ngắt nghỉ ngẫu nhiên 1.8s - 3.8s giữa các cursor phân trang. Tự động tạm dừng nếu nhận status code 429 hoặc checkpoint.

4. CSDL SQLite cục bộ (Client-side):
- Kích hoạt chế độ WAL: `PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;`.
- Bảng `monitored_posts`:
  * `post_id` (TEXT PK), `post_url` (TEXT), `author_name` (TEXT), `content_preview` (TEXT), `total_comments_crawled` (INT DEFAULT 0), `total_orders_detected` (INT DEFAULT 0), `total_phones_detected` (INT DEFAULT 0), `status` (TEXT: 'ACTIVE', 'PAUSED', 'COMPLETED', 'ERROR'), `crawl_interval_minutes` (INT DEFAULT 30), `last_cursor` (TEXT), `last_crawled_at` (INT), `created_at` (INT).
- Bảng `comments`:
  * `id` (TEXT PK), `post_id` (TEXT FK), `parent_comment_id` (TEXT), `author_id` (TEXT), `author_name` (TEXT), `author_url` (TEXT), `content` (TEXT), `phone_numbers` (TEXT), `intent_tag` (TEXT: '[Chốt đơn]', '[Hỏi giá]', '[Khiếu nại]', '[Spam]'), `created_at` (INT).
- Thực thi ghi dữ liệu theo từng Batch Transaction (500–1000 items/commit) để không gây giật lag giao diện.

5. Màn hình quản lý danh sách Post theo dõi (Watchlist Tracker):
- Giao diện Dashboard (React):
  * Thanh công cụ: Input nhập link Facebook (chuẩn hóa các dạng URL sang Post ID), chọn chu kỳ quét (15m, 30m, 1h), nút "Thêm vào theo dõi".
  * Bảng hiển thị: Preview nội dung, tác giả, huy hiệu thống kê tổng comment / số điện thoại / đơn hàng phát hiện, Tag trạng thái, Switch bật/tắt quét, nút "Quét ngay", nút "Xuất Excel" và "Xem chi tiết comment".
  * Cập nhật thời gian thực qua Tauri Events (`app.emit`) khi luồng nền cào được dữ liệu mới.
- Logic Worker Rust (Incremental Crawling):
  * Một background task độc lập định kỳ kiểm tra các bài viết `ACTIVE` đến hạn quét.
  * Cơ chế quét gia tăng (Incremental): Chỉ cào các comment mới sinh ra (dựa vào `last_cursor` hoặc mốc `created_at` lớn hơn comment mới nhất trong DB), gặp dữ liệu cũ thì dừng ngay để tiết kiệm tài nguyên và bảo vệ tài khoản.

================================================================================
KẾT QUẢ ĐẦU RA CẦN CUNG CẤP:
================================================================================
1. Phía Server (Python FastAPI):
   - `server/models.py`: Khai báo bảng SQLAlchemy PostgreSQL.
   - `server/security.py`: Logic ký chữ ký số Ed25519 và quản lý key.
   - `server/main.py`: Các route `/login`, `/heartbeat`, `/sync`.
2. Phía Client (Rust / Tauri v2):
   - `Cargo.toml`: Cấu hình dependencies, release profile chống decompile (LTO, strip, panic abort).
   - `src-tauri/src/hwid.rs` & `license.rs`: Thu thập HWID, verify chữ ký Ed25519 bằng Public Key, background loop Heartbeat 5 phút/lần.
   - `src-tauri/src/db.rs`: Khởi tạo SQLite WAL mode, schema và hàm batch insert có transaction.
   - `src-tauri/src/crawler.rs`: Cấu trúc gọi GraphQL nội bộ, session headers, jitter sleep, regex bóc tách SĐT và phân loại intent.
   - `src-tauri/src/tracker.rs`: Luồng ngầm định kỳ cào gia tăng (Incremental Crawl) các post trong Watchlist.
   - `src/components/PostTrackerDashboard.tsx`: Component React hiển thị bảng điều khiển quản lý post theo dõi.