Bạn là một Principal Frontend Architect và Senior SEO/Performance Engineer. Hãy thiết kế cấu trúc dự án và viết toàn bộ mã nguồn lõi cho Web Client của hệ thống phần mềm thương mại cào dữ liệu mạng xã hội & quản lý bản quyền.

### 1. TECH STACK & CẤU HÌNH HỆ THỐNG
- Runtime: Node.js 24 LTS (tận dụng native module loading, V8 engine mới nhất).
- Core Framework: Vite 5+ kết hợp React 18+ (TypeScript strict mode).
- Tối ưu SEO & Social Sharing (Vite SSG/Prerender):
  * Tích hợp `react-helmet-async` để quản lý thẻ `<title>`, `<meta name="description">`, canonical, `<meta property="og:...">` và `<meta name="twitter:...">` động theo trang và theo ngôn ngữ.
  * Cấu hình Prerender (`vite-plugin-prerender` hoặc tương đương) trong `vite.config.ts` để render sẵn mã HTML tĩnh (Static HTML snapshot) cho các route công khai (`/`, `/pricing`, `/features`) lúc chạy build, đảm bảo Google Bot và Social Bot (Facebook/Zalo/Telegram) cào được nội dung và ảnh preview mà không cần thực thi JavaScript.
- UI Frameworks kết hợp:
  * Material UI (MUI v6): Sử dụng cho DataGrid, Modal, Dialog, Form Controls, Badges ở trang quản trị (Dashboard).
  * TailwindCSS (v3): Dùng cho Layouts, Responsive, Landing Page, Background Gradients và Animations.
  * Cấu hình CSS Injection Order: Sử dụng `StyledEngineProvider injectFirst` để class utility của TailwindCSS có thể override style mặc định của MUI mượt mà.
- Đa ngôn ngữ (i18n): `i18next` + `react-i18next` hỗ trợ Tiếng Việt (`vi`) và Tiếng Anh (`en`), lưu trạng thái vào `localStorage` và tự động cập nhật thẻ `<html lang="...">`.
- Theme (Dark/Light Mode):
  * Đồng bộ class `dark` của `<html>` (Tailwind) với `ThemeProvider` (MUI Palette).
  * Nút ThemeToggle lưu cấu hình vào `localStorage`.
- HTTP Client: `axios` cấu hình sẵn base URL và interceptors gắn JWT token.

---

### 2. KIẾN TRÚC PHÂN HỆ VÀ TÍNH NĂNG

#### PHÂN HỆ 1: LANDING PAGE CÔNG KHAI (SEO OPTIMIZED)
Dùng TailwindCSS kết hợp các thẻ ngữ nghĩa chuẩn SEO (`<header>`, `<main>`, `<section>`, `<h1>`, `<h2>`):
1. SEO & Head Meta:
   - Thẻ Meta title/description chuẩn SEO theo từ khóa: cào bình luận Facebook, trích xuất số điện thoại, quản lý đơn hàng tự động.
   - Thẻ Open Graph (`og:title`, `og:image`, `og:url`, `og:type`) phục vụ chia sẻ link mạng xã hội.
2. Header / Navbar:
   - Logo, Menu điều hướng (Tính năng, Bảng giá, Tải App).
   - Nút đổi Theme (Light/Dark), Nút chuyển ngôn ngữ (EN/VI).
   - Nút CTA "Bảng điều khiển / Đăng nhập".
3. Hero Section:
   - Headline chuẩn H1 tối ưu chuyển đổi, mô tả giá trị cốt lõi (nhẹ, nhanh, không checkpoint).
   - Nút CTA kép: "Tải bản Windows / macOS" và "Mua bản quyền".
   - Mockup giao diện Desktop App dạng Dark card hiện đại.
4. Feature Showcase (H2 + Grid Cards):
   - Cào GraphQL siêu tốc, Bóc tách SĐT/đơn hàng bằng AI & Regex, Quản lý Watchlist theo dõi bài viết, Lưu trữ SQLite cục bộ.
5. Pricing Table (Bảng giá bản quyền):
   - Các gói: Gói Tháng, Gói Năm, Gói Vĩnh Viễn (Lifetime).
   - Thẻ so sánh quyền lợi: Số thiết bị (HWID), cập nhật tính năng, hỗ trợ cloud sync.
6. Footer: Thông tin bản quyền, sitemap, điều khoản và kênh hỗ trợ.

#### PHÂN HỆ 2: PORTAL QUẢN LÝ BẢN QUYỀN (LICENSE DASHBOARD)
Dùng MUI DataGrid kết hợp Tailwind Layout (Bảo vệ bởi Auth/No-index):
1. Màn hình Đăng nhập (Auth):
   - Form đăng nhập Email/Password với validation.
2. Thống kê tổng quan:
   - Cards thống kê: Tổng số License, Số thiết bị đang gắn (Active HWID), Hạn sử dụng.
3. Bảng quản lý License & Thiết bị:
   - MUI DataGrid hiển thị: License Key (ẩn/hiện ký tự, copy 1-click), Loại gói, Hạn dùng, Số máy kích hoạt / Tối đa, Trạng thái (Active/Expired/Revoked).
   - Dialog "Danh sách thiết bị": Hiển thị tên máy tính, HWID băm SHA256, thời điểm Heartbeat cuối và nút "Thu hồi thiết bị / Reset HWID".
4. Kích hoạt thêm: Form nhập mã bản quyền mới mua để kích hoạt vào tài khoản.

---

### 3. CẤU TRÚC THƯ MỤC
```text
src/
├── api/             # Axios instance & endpoints gọi sang FastAPI
│   ├── auth.ts
│   └── license.ts
├── assets/          # Static files, og-image.png, logo
├── components/      # Common UI components
│   ├── Navbar.tsx
│   ├── Footer.tsx
│   ├── ThemeToggle.tsx
│   ├── LanguageSwitcher.tsx
│   ├── SeoMeta.tsx  # Component bọc Helmet để render thẻ meta/OG chuẩn
│   └── ProtectedRoute.tsx
├── contexts/        # ThemeContext, AuthContext
├── i18n/            # Cấu hình i18next & locales
│   ├── index.ts
│   └── locales/
│       ├── en.json
│       └── vi.json
├── layouts/         # PublicLayout, DashboardLayout
├── pages/           # LandingPage, Login, LicenseDashboard
├── theme/           # MUI Theme config (Dark/Light palette)
└── types/           # Interface TypeScript (License, Device, User)