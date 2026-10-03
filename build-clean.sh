#!/usr/bin/env bash
set -e

echo "=========================================================="
echo " 🧹 DỌN DẸP CÁC FILE BUILD CŨ & GIẢI PHÓNG DUNG LƯỢNG"
echo "=========================================================="

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLIENT_DIR="$ROOT_DIR/pc_client_app"

# Tính toán dung lượng trước khi dọn dẹp
calc_size() {
    local target_dir="$1"
    if [ -d "$target_dir" ]; then
        du -sh "$target_dir" 2>/dev/null | awk '{print $1}'
    else
        echo "0B"
    fi
}

TARGET_SIZE=$(calc_size "$CLIENT_DIR/src-tauri/target")
DIST_SIZE=$(calc_size "$CLIENT_DIR/dist")

echo "[*] Dung lượng build hiện tại:"
echo "    - Rust target: $TARGET_SIZE"
echo "    - Web dist:    $DIST_SIZE"
echo "----------------------------------------------------------"

# 1. Dọn dẹp thư mục build Frontend (Vite / React)
echo "[1/4] Xoá thư mục dist của Frontend..."
if [ -d "$CLIENT_DIR/dist" ]; then
    rm -rf "$CLIENT_DIR/dist"
    echo "      ✓ Đã xoá $CLIENT_DIR/dist"
fi

if [ -d "$ROOT_DIR/web_client/dist" ]; then
    rm -rf "$ROOT_DIR/web_client/dist"
    echo "      ✓ Đã xoá $ROOT_DIR/web_client/dist"
fi

# 2. Dọn dẹp thư mục build Rust / Tauri (target)
echo "[2/4] Dọn dẹp thư mục build Rust & Tauri (Cargo target)..."
if [ -d "$CLIENT_DIR/src-tauri/target" ]; then
    if command -v cargo &>/dev/null; then
        (cd "$CLIENT_DIR/src-tauri" && cargo clean 2>/dev/null) || true
    fi
    rm -rf "$CLIENT_DIR/src-tauri/target"
    echo "      ✓ Đã xoá sạch $CLIENT_DIR/src-tauri/target (giải phóng ~$TARGET_SIZE)"
fi

# Xoá các thư mục target phát sinh khác nếu có
find "$ROOT_DIR" -name "target" -type d -not -path "*/.*" -exec rm -rf {} + 2>/dev/null || true

# 3. Dọn dẹp cache Python (__pycache__, *.pyc)
echo "[3/4] Dọn dẹp bộ nhớ đệm Python..."
find "$ROOT_DIR" -type d -name "__pycache__" -exec rm -rf {} + 2>/dev/null || true
find "$ROOT_DIR" -type f -name "*.py[cod]" -delete 2>/dev/null || true
find "$ROOT_DIR" -type d -name ".pytest_cache" -exec rm -rf {} + 2>/dev/null || true
find "$ROOT_DIR" -type d -name ".mypy_cache" -exec rm -rf {} + 2>/dev/null || true
find "$ROOT_DIR" -type d -name ".ruff_cache" -exec rm -rf {} + 2>/dev/null || true
echo "      ✓ Đã dọn dẹp các cache Python"

# 4. Dọn dẹp log và file hệ thống tạm
echo "[4/4] Dọn dẹp logs và file tạm hệ thống..."
find "$ROOT_DIR" -type f -name "*.log" -delete 2>/dev/null || true
find "$ROOT_DIR" -type f -name ".DS_Store" -delete 2>/dev/null || true
echo "      ✓ Đã xoá log và file tạm"

# Tùy chọn: Xoá sâu node_modules khi truyền cờ --deep hoặc --all
if [ "$1" == "--deep" ] || [ "$1" == "--all" ]; then
    echo "----------------------------------------------------------"
    echo "[Deep Clean] Xoá node_modules..."
    if [ -d "$CLIENT_DIR/node_modules" ]; then
        rm -rf "$CLIENT_DIR/node_modules"
        echo "      ✓ Đã xoá $CLIENT_DIR/node_modules"
    fi
fi

echo "=========================================================="
echo " ✨ ĐÃ HOÀN TẤT DỌN DẸP TOÀN BỘ FILE BUILD CŨ!"
echo " Bạn có thể chạy lại './build-client.sh' hoặc './run-dev.sh' bất kỳ lúc nào."
echo "=========================================================="
