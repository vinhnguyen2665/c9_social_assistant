#!/usr/bin/env bash
set -e

# Load NVM and use Node 24
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
nvm use 24

echo "=========================================================="
echo " Starting C9 Social Assistant Development Environment"
echo " Node Version: $(node -v)"
echo " Python Version: $(python3 --version)"
echo " Cargo Version: $(cargo --version)"
echo "=========================================================="

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Auto-free ports 8000 (Backend) and 1420 (Vite) if previously occupied
for PORT in 8000 1420; do
    PID=$(lsof -ti :$PORT 2>/dev/null || true)
    if [ -n "$PID" ]; then
        echo "[!] Port $PORT is occupied by PID $PID. Freeing port..."
        kill -9 $PID 2>/dev/null || true
    fi
done

# 1. Start Backend in background
echo "[1/2] Starting Python FastAPI Backend on http://127.0.0.1:8000..."
cd "$ROOT_DIR/backend"
if [ ! -d ".venv" ]; then
    python3 -m venv .venv
    .venv/bin/pip install -r requirements.txt
    .venv/bin/python seed.py
fi
.venv/bin/python -m uvicorn server.main:app --port 8000 --reload &
BACKEND_PID=$!

cleanup() {
    echo "Stopping backend server (PID $BACKEND_PID)..."
    kill $BACKEND_PID 2>/dev/null || true
}
trap cleanup EXIT

# 2. Start Desktop App (or Web if --web flag is provided)
cd "$ROOT_DIR/pc_client_app"

if [ "$1" == "--web" ]; then
    echo "[2/2] Starting Vite Frontend on http://localhost:1420 (Web mode)..."
    npm run dev
else
    echo "[2/2] Launching Native Desktop Application Window (Tauri dev)..."
    echo "      (Tip: Use './run-dev.sh --web' if you only want browser mode)"
    npm run tauri dev
fi
