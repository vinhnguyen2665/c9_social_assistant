#!/usr/bin/env bash
set -e

# Load NVM and use Node 24
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
nvm use 24 2>/dev/null || true

echo "=========================================================="
echo " 🔨 Building C9 Social Assistant PC Client Desktop App"
echo " Node Version:   $(node -v 2>/dev/null || echo 'Not found')"
echo " Cargo Version:  $(cargo --version 2>/dev/null || echo 'Not found')"
echo "=========================================================="

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLIENT_DIR="$ROOT_DIR/pc_client_app"

cd "$CLIENT_DIR"

if [ ! -d "node_modules" ]; then
    echo "[1/3] Cài đặt dependencies (npm install)..."
    npm install
else
    echo "[1/3] Dependencies node_modules đã sẵn sàng."
fi

BUILD_MODE="release"
BUILD_CMD="npm run build:app"

if [ "$1" == "--debug" ]; then
    BUILD_MODE="debug"
    BUILD_CMD="npm run build:debug"
    echo "[2/3] Chế độ build: DEBUG (nhanh hơn, không tối ưu kích thước / LTO)"
else
    echo "[2/3] Chế độ build: RELEASE (Anti-reversing, LTO, Opt-level 'z', Strip symbols)"
fi

echo "[3/3] Bắt đầu biên dịch ứng dụng Desktop Tauri ($BUILD_MODE)..."
$BUILD_CMD

echo ""
echo "=========================================================="
echo " 🎉 BUILD HOÀN TẤT THÀNH CÔNG!"
echo " Thư mục chứa gói cài đặt ứng dụng:"
if [ "$BUILD_MODE" == "release" ]; then
    echo " 📂 $CLIENT_DIR/src-tauri/target/release/bundle/"
else
    echo " 📂 $CLIENT_DIR/src-tauri/target/debug/bundle/"
fi
echo "=========================================================="
