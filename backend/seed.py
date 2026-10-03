import asyncio
import os
import sys
from datetime import datetime, timezone, timedelta
from pathlib import Path

# Add backend directory to sys.path
sys.path.insert(0, str(Path(__file__).parent))

from server.database import engine, Base, AsyncSessionLocal
from server.models import License
from server.security import security_manager

async def seed():
    print("=" * 60)
    print("C9 Social Assistant - License Key & Seed Generator")
    print("=" * 60)

    # 1. Initialize keys
    security_manager.ensure_keys()
    pub_hex = security_manager.get_public_key_raw_hex()
    pub_b64 = security_manager.get_public_key_raw_base64()
    print(f"[*] Ed25519 Public Key (Hex 32-bytes): {pub_hex}")
    print(f"[*] Ed25519 Public Key (Base64):     {pub_b64}")

    # 2. Create tables
    async with engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)
    print("[*] Database tables verified.")

    # 3. Create demo licenses
    async with AsyncSessionLocal() as session:
        from sqlalchemy import select
        stmt = select(License).where(License.license_key == "C9-PRO-2026-VIP")
        res = await session.execute(stmt)
        existing = res.scalar_one_or_none()

        if not existing:
            demo_license = License(
                license_key="C9-PRO-2026-VIP",
                client_name="Khách hàng VIP (Bản quyền 1 năm)",
                max_devices=3,
                expires_at=datetime.now(timezone.utc) + timedelta(days=365),
                is_active=True
            )
            session.add(demo_license)
            await session.commit()
            print("[+] Đã tạo License mẫu: C9-PRO-2026-VIP (Hạn: 365 ngày, tối đa 3 thiết bị)")
        else:
            print("[*] License C9-PRO-2026-VIP đã tồn tại.")

    print("=" * 60)
    print("Hoàn tất khởi tạo Server!")

if __name__ == "__main__":
    asyncio.run(seed())
