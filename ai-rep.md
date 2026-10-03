Đã thực hiện đầy đủ toàn bộ kiến trúc và mã nguồn theo đúng tài liệu 

todo.md
 trên nền tảng Node v24.21.0 (nvm use 24).

1. Phía Server Backend (Python FastAPI + PostgreSQL/Async)
Thư mục: 

backend/


server/models.py
: Khai báo đầy đủ các bảng SQLAlchemy 2.0 Async:
licenses: Khóa bản quyền UUID, số lượng thiết bị tối đa (max_devices), ngày hết hạn, trạng thái.
activated_devices: HWID (SHA-256), tên thiết bị, session token 15 phút, heartbeat gần nhất.
cloud_posts & cloud_comments: Nhận dữ liệu backup/đồng bộ từ client.


server/security.py
: Cơ chế mật mã bất đối xứng Ed25519 (cryptography). Server giữ Private Key để ký signed_payload (hwid, session_token, exp, license_key).


server/main.py
:
POST /api/v1/license/login: Xác thực key, kiểm soát HWID, cấp session token & chữ ký Ed25519.
POST /api/v1/license/heartbeat: Duy trì phiên làm việc 5 phút/lần, gia hạn thêm 15 phút, trả về HTTP 403 nếu vi phạm.
POST /api/v1/sync/batch: Bulk upsert bài viết và bình luận lên Cloud.


seed.py
: Tự động sinh cặp khóa Ed25519 và tạo sẵn bản quyền mẫu: C9-PRO-2026-VIP (Hạn 1 năm, tối đa 3 thiết bị).
2. Phía Client Desktop App (Tauri v2 - Rust & React + TailwindCSS)
Thư mục: 

pc_client_app/


src-tauri/Cargo.toml
: Cấu hình Profile Release chống dịch ngược (opt-level = "z", lto = true, codegen-units = 1, panic = "abort", strip = true).


src-tauri/src/hwid.rs
: Thu thập phần cứng (CPU ID, Serial bo mạch chủ, Disk UUID) và băm SHA-256 thành mã HWID duy nhất.


src-tauri/src/license.rs
:
Strict Online-Only: Nhúng Ed25519 Public Key để verify chữ ký số của server.
Background Heartbeat Worker ngầm 5 phút/lần: Tự động khóa app và phát sự kiện license_locked nếu mất mạng quá 2 chu kỳ (10 phút) hoặc server trả mã 403.
Toàn bộ chuỗi nhạy cảm (Public Key, URL) được bảo vệ bằng macro obfstring.


src-tauri/src/db.rs
: SQLite cục bộ với PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;, ghi dữ liệu bình luận theo Batch Transaction (500 items/commit).


src-tauri/src/crawler.rs
:
Gọi trực tiếp GraphQL nội bộ của Facebook qua HTTP/2 bằng session cookies (c_user, xs, datr, fr, fb_dtsg).
Jitter sleep ngẫu nhiên từ 1.8s - 3.8s giữa các cursor phân trang.
Tự động bóc tách số điện thoại di động Việt Nam và phân loại ý định: [Chốt đơn], [Hỏi giá], [Khiếu nại], [Spam].


src-tauri/src/tracker.rs
: Luồng ngầm định kỳ cào gia tăng (Incremental Crawl), dừng ngay khi gặp dữ liệu cũ, phát sự kiện thời gian thực post_updated lên React.


src/components/PostTrackerDashboard.tsx
: Màn hình Dashboard quản lý bài viết theo dõi:
Input chuẩn hóa URL sang Post ID, chọn chu kỳ (15m, 30m, 1h).
Huy hiệu thống kê tổng bình luận, SĐT, đơn hàng phát hiện.
Nút "Quét ngay", Switch bật/tắt, "Xuất Excel" (CSV hỗ trợ tiếng Việt) và "Xem chi tiết comment".


src/components/CommentsModal.tsx
: Xem chi tiết comment, bộ lọc Intent, 1-click sao chép SĐT.


src/components/LicenseLockModal.tsx
: Giao diện khóa ứng dụng và kích hoạt bản quyền trực tuyến.
3. Cách khởi chạy nhanh
Bạn có thể chạy script 

run-dev.sh
:

bash
./run-dev.sh
Hoặc chạy từng phân hệ:

bash
# Terminal 1 - Chạy Backend FastAPI
cd backend
./.venv/bin/python -m uvicorn server.main:app --port 8000 --reload
# Terminal 2 - Chạy Desktop Client (Node 24)
export NVM_DIR="$HOME/.nvm" && [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh" && nvm use 24
cd pc_client_app
npm run dev          # hoặc: npm run tauri dev
License Key dùng thử đã tạo sẵn: C9-PRO-2026-VIP
License Server URL: http://127.0.0.1:8000