Bạn là một Senior AI Systems Architect và Full-stack Engineer. Hãy thiết kế cấu trúc và viết code nâng cấp tính năng: "Phân loại bình luận đa chế độ AI (Cloud & Local) kèm Màn hình Cài đặt (Settings)" cho Client Desktop App (Tauri v2 - Rust Backend & React/TypeScript Frontend).

================================================================================
MỤC TIÊU NÂNG CẤP
================================================================================
Người dùng có thể linh hoạt lựa chọn cơ chế AI để phân loại bình luận (Intent, Sentiment, Trích xuất SĐT/Đơn hàng):
1. CLOUD AI: Dùng Google AI Studio API (Gemini 1.5/2.0 Flash) - Nhanh, chuẩn xác tiếng Việt, không tốn tài nguyên máy.
2. LOCAL AI (Chạy cục bộ trên máy, hoàn toàn offline/private):
   - Option A (LLM Local qua Ollama / vLLM / OpenAI-compatible API): Cho phép nhập Base URL (VD: `http://localhost:11434/v1`), tự động fetch danh sách model khả dụng qua endpoint `/v1/models`, chọn model mặc định `Qwen 2.5-1.5B` hoặc tùy chọn đổi sang các model khác.
   - Option B (Mô hình NLP siêu nhẹ PhoBERT / ViBERT / ONNX Runtime): Chạy phân loại Sentiment & Intent trực tiếp bằng mô hình nhúng cục bộ siêu nhẹ (vài chục MB đến vài trăm MB) mà không cần cấu hình server LLM.

================================================================================
1. CẤU TRÚC LƯU TRỮ CẤU HÌNH (SETTINGS STORAGE)
================================================================================
Lưu cấu hình an toàn trong SQLite hoặc tệp JSON trong AppData của người dùng (`app_settings`):
- `ai_provider`: "GOOGLE_STUDIO" | "LOCAL_LLM" | "LOCAL_NLP"
- `google_studio_api_key`: string (Mã hóa an toàn lúc lưu trữ)
- `google_model`: string (Mặc định: "gemini-1.5-flash")
- `local_llm_base_url`: string (Mặc định: "http://localhost:11434/v1" hoặc "http://localhost:8000/v1")
- `local_llm_model`: string (Mặc định: "qwen2.5:1.5b")
- `local_llm_api_key`: string (Tùy chọn, dùng nếu vLLM có auth)
- `local_nlp_model_path`: string (Đường dẫn tới file model ONNX của PhoBERT/ViBERT)
- `batch_size`: number (Số lượng comment gom lại phân tích 1 lần, mặc định 30)

================================================================================
2. GIAO DIỆN MÀN HÌNH SETTINGS (REACT + SHADCN / TAILWIND)
================================================================================
Xây dựng component `AiSettingsModal.tsx` hoặc `SettingsView.tsx`:
- Tabs hoặc Radio Groups chọn nhà cung cấp AI:
  
  [1] Tab Google AI Studio:
  - Input: Google AI Studio API Key (có toggle ẩn/hiện, nút "Test kết nối").
  - Dropdown chọn Model: `gemini-1.5-flash`, `gemini-2.0-flash`, `gemini-1.5-pro`.
  - Link hướng dẫn người dùng lấy key miễn phí từ Google AI Studio.

  [2] Tab Local LLM (Ollama / vLLM):
  - Input: Base URL (VD: `http://localhost:11434/v1`).
  - Nút "Kiểm tra & Tải danh sách Model": Gửi request tới `{base_url}/models` để lấy toàn bộ model đang cài trong Ollama/vLLM và hiển thị vào Dropdown Select.
  - Dropdown chọn Model: Tự động điền danh sách, ưu tiên chọn `qwen2.5:1.5b` hoặc `qwen2.5:3b`.
  - Thông số nâng cao: Temperature (0.1 - 0.7), Max Tokens.

  [3] Tab Mô hình NLP nhẹ (PhoBERT / ViBERT):
  - Card thông tin: Chạy inference ONNX trực tiếp trên CPU, siêu nhẹ, chỉ phân loại Intent và Sentiment cơ bản.
  - Trạng thái Model: Đã tải (Ready) hoặc nút "Tải model PhoBERT ONNX (~120MB)" về thư mục ứng dụng.

- Nút "Lưu Cấu Hình" (Save Settings) và thông báo Toast xác nhận.

================================================================================
3. CORE AI ENGINE TRONG RUST BACKEND (TAURI COMMANDS)
================================================================================
Tạo một trait/module thống nhất `AiClassifier`:
`pub trait Classifier { async fn classify_batch(&self, comments: Vec<RawComment>) -> Result<Vec<ClassifiedComment>, AppError>; }`

Triển khai 3 Provider:
1. `GoogleStudioProvider`:
   - Dùng `reqwest` gọi endpoint `https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={api_key}`.
   - Bật cờ `response_schema` hoặc `response_mime_type: "application/json"` để ép Gemini trả về chuẩn xác cấu trúc JSON.
2. `LocalLlmProvider` (Ollama / vLLM):
   - Chuẩn hóa request theo định dạng OpenAI `/v1/chat/completions`.
   - Command `fetch_local_models(base_url: String)`: Gọi `GET {base_url}/models` parse danh sách `id` trả về cho React UI.
   - Gửi system prompt bằng tiếng Anh yêu cầu đầu ra JSON thuần túy (tối ưu tốc độ suy luận cho Qwen 2.5-1.5B).
3. `LocalNlpProvider` (PhoBERT / ViBERT qua `ort` - ONNX Runtime Rust):
   - Tokenize tiếng Việt bằng thư viện nhẹ hoặc pre-computed BPE tokenizer.
   - Chạy inference qua file `.onnx` trên CPU để lấy nhãn phân loại Sentiment (Positive/Negative/Neutral) và Intent Tag.
   - Kết hợp Regex để bóc tách Số điện thoại độc lập.

================================================================================
4. TAXONOMY VÀ PROMPT DÙNG CHUNG CHO LLM (CLOUD & LOCAL)
================================================================================
Sử dụng chung cấu trúc System Prompt sau:

"You are an NLP social commerce classifier. Classify the input comments array into JSON strictly matching this schema:
[
  {
    "id": "string",
    "primary_intent": "ORDER_CLOSING" | "PRODUCT_INQUIRY" | "FEEDBACK_COMPLAINT" | "CONTENT_IDEA" | "CASUAL_CHAT" | "SPAM_PROMO",
    "sentiment": "POSITIVE" | "NEGATIVE" | "NEUTRAL",
    "action_required": boolean,
    "phone_numbers": ["string"],
    "extracted_entities": { "variant": "string", "quantity": number, "address": "string" } | null
  }
]
Output ONLY valid JSON. No prose."

================================================================================
KẾT QUẢ ĐẦU RA CẦN CUNG CẤP
================================================================================
1. `src-tauri/src/ai/settings.rs`: Cấu trúc dữ liệu cấu hình, lệnh lưu/đọc config từ SQLite.
2. `src-tauri/src/ai/mod.rs`: Trait `Classifier` và hàm factory router chọn đúng provider dựa theo setting.
3. `src-tauri/src/ai/google.rs`: Provider gọi Google Studio Gemini API (hỗ trợ Structured JSON).
4. `src-tauri/src/ai/local_llm.rs`: Provider gọi Ollama/vLLM/OpenAI-compatible kèm hàm fetch danh sách model.
5. `src-tauri/src/ai/nlp_onnx.rs`: Cấu trúc loader và inference PhoBERT/ViBERT ONNX.
6. `src/components/AiSettingsModal.tsx`: Giao diện React hoàn chỉnh gồm các tab cấu hình, fetch model Ollama tự động và test connection.
