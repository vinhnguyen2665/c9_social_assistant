import os
import secrets
from datetime import datetime, timezone, timedelta
from contextlib import asynccontextmanager
from typing import List

from fastapi import FastAPI, Depends, HTTPException, status
from fastapi.middleware.cors import CORSMiddleware
from sqlalchemy.ext.asyncio import AsyncSession
from sqlalchemy import select, func

from .database import engine, Base, get_db
from .models import License, ActivatedDevice, CloudPost, CloudComment, utc_now
from .security import security_manager
from .schemas import (
    LoginRequest,
    HeartbeatRequest,
    LicenseResponse,
    SyncBatchRequest,
    SyncBatchResponse
)

SESSION_TTL_MINUTES = int(os.getenv("SESSION_TOKEN_TTL_MINUTES", "15"))

@asynccontextmanager
async def lifespan(app: FastAPI):
    # Ensure tables exist on startup
    async with engine.begin() as conn:
        await conn.run_sync(Base.metadata.create_all)
    # Ensure Ed25519 keys exist
    security_manager.ensure_keys()

    # Ensure default license C9-PRO-2026-VIP exists
    from .database import AsyncSessionLocal
    async with AsyncSessionLocal() as session:
        stmt = select(License).where(License.license_key == "C9-PRO-2026-VIP")
        res = await session.execute(stmt)
        if not res.scalar_one_or_none():
            demo_license = License(
                license_key="C9-PRO-2026-VIP",
                client_name="Khách hàng VIP (Bản quyền 1 năm)",
                max_devices=3,
                expires_at=datetime.now(timezone.utc) + timedelta(days=365),
                is_active=True
            )
            session.add(demo_license)
            await session.commit()
            print("[+] Tự động khởi tạo License: C9-PRO-2026-VIP")
    yield

app = FastAPI(
    title="C9 Social Assistant License & Cloud Server",
    description="Online License Verification with Ed25519 signatures and Cloud Sync",
    version="1.0.0",
    lifespan=lifespan
)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


@app.get("/health")
async def health_check():
    return {
        "status": "ok",
        "service": "c9_social_assistant_server",
        "time": utc_now().isoformat()
    }


@app.get("/api/v1/license/public-key")
async def get_public_key():
    """Returns the raw Ed25519 public key in Hex and Base64 format for client embedding."""
    return {
        "format": "ed25519_raw_32bytes",
        "hex": security_manager.get_public_key_raw_hex(),
        "base64": security_manager.get_public_key_raw_base64()
    }


@app.post("/api/v1/license/login", response_model=LicenseResponse)
async def login(req: LoginRequest, db: AsyncSession = Depends(get_db)):
    """
    Authenticate license key and register/verify HWID.
    Returns Ed25519 signed session payload valid for 15 minutes.
    """
    # 1. Fetch license
    stmt = select(License).where(License.license_key == req.license_key.strip())
    res = await db.execute(stmt)
    license_obj = res.scalar_one_or_none()

    if not license_obj:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Bản quyền không tồn tại trong hệ thống."
        )

    if not license_obj.is_active:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Bản quyền này đã bị vô hiệu hóa hoặc thu hồi."
        )

    now = utc_now()
    expires_at = license_obj.expires_at
    if expires_at.tzinfo is None:
        expires_at = expires_at.replace(tzinfo=timezone.utc)

    if expires_at <= now:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail=f"Bản quyền đã hết hạn vào ngày {expires_at.strftime('%d/%m/%Y %H:%M:%S')}."
        )

    # 2. Check activated devices
    dev_stmt = select(ActivatedDevice).where(
        ActivatedDevice.license_id == license_obj.id,
        ActivatedDevice.hwid == req.hwid
    )
    dev_res = await db.execute(dev_stmt)
    device_obj = dev_res.scalar_one_or_none()

    if device_obj:
        if device_obj.is_revoked:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail="Thiết bị này đã bị khóa khỏi bản quyền."
            )
    else:
        # Check max devices limit
        count_stmt = select(func.count(ActivatedDevice.id)).where(
            ActivatedDevice.license_id == license_obj.id,
            ActivatedDevice.is_revoked == False
        )
        count_res = await db.execute(count_stmt)
        active_count = count_res.scalar_one()

        if active_count >= license_obj.max_devices:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail=f"Bản quyền đã đạt giới hạn tối đa ({license_obj.max_devices} thiết bị). Vui lòng liên hệ quản trị viên."
            )

        # Register new device
        device_obj = ActivatedDevice(
            license_id=license_obj.id,
            hwid=req.hwid,
            device_name=req.device_name or "Desktop Device"
        )
        db.add(device_obj)

    # 3. Generate session token & 15-minute expiration
    session_token = secrets.token_urlsafe(32)
    exp_timestamp = int((now + timedelta(minutes=SESSION_TTL_MINUTES)).timestamp())

    device_obj.current_session_token = session_token
    device_obj.last_heartbeat = now
    if req.device_name:
        device_obj.device_name = req.device_name

    await db.commit()
    await db.refresh(device_obj)

    # 4. Sign payload with Ed25519
    payload_to_sign = {
        "hwid": req.hwid,
        "session_token": session_token,
        "exp": exp_timestamp,
        "license_key": license_obj.license_key
    }
    signed = security_manager.sign_payload(payload_to_sign)

    return LicenseResponse(
        success=True,
        message="Kích hoạt bản quyền thành công.",
        client_name=license_obj.client_name,
        expires_at=license_obj.expires_at.isoformat(),
        session_token=session_token,
        signed_payload=signed
    )


@app.post("/api/v1/license/heartbeat", response_model=LicenseResponse)
async def heartbeat(req: HeartbeatRequest, db: AsyncSession = Depends(get_db)):
    """
    Heartbeat check from client every 5 minutes.
    Extends session expiration by 15 minutes if valid.
    """
    # 1. Fetch license
    stmt = select(License).where(License.license_key == req.license_key.strip())
    res = await db.execute(stmt)
    license_obj = res.scalar_one_or_none()

    if not license_obj or not license_obj.is_active:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Bản quyền không hợp lệ hoặc đã bị khóa."
        )

    now = utc_now()
    expires_at = license_obj.expires_at
    if expires_at.tzinfo is None:
        expires_at = expires_at.replace(tzinfo=timezone.utc)

    if expires_at <= now:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Bản quyền đã hết hạn."
        )

    # 2. Verify device and session token
    dev_stmt = select(ActivatedDevice).where(
        ActivatedDevice.license_id == license_obj.id,
        ActivatedDevice.hwid == req.hwid
    )
    dev_res = await db.execute(dev_stmt)
    device_obj = dev_res.scalar_one_or_none()

    if not device_obj or device_obj.is_revoked:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Thiết bị chưa được đăng ký hoặc đã bị thu hồi."
        )

    if device_obj.current_session_token != req.session_token:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Phiên làm việc (session token) không hợp lệ hoặc đã bị ghi đè bởi phiên khác."
        )

    # 3. Renew session token expiration
    exp_timestamp = int((now + timedelta(minutes=SESSION_TTL_MINUTES)).timestamp())
    device_obj.last_heartbeat = now
    await db.commit()

    payload_to_sign = {
        "hwid": req.hwid,
        "session_token": req.session_token,
        "exp": exp_timestamp,
        "license_key": license_obj.license_key
    }
    signed = security_manager.sign_payload(payload_to_sign)

    return LicenseResponse(
        success=True,
        message="Duy trì phiên làm việc thành công.",
        client_name=license_obj.client_name,
        expires_at=expires_at.isoformat(),
        session_token=req.session_token,
        signed_payload=signed
    )


@app.post("/api/v1/sync/batch", response_model=SyncBatchResponse)
async def sync_batch(req: SyncBatchRequest, db: AsyncSession = Depends(get_db)):
    """
    Receives posts and comments batch from client to persist in cloud database.
    """
    # 1. Authorize session
    stmt = select(License).where(License.license_key == req.license_key.strip())
    res = await db.execute(stmt)
    license_obj = res.scalar_one_or_none()

    now = utc_now()
    if not license_obj or not license_obj.is_active:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Bản quyền không hợp lệ để thực hiện đồng bộ."
        )

    exp_at = license_obj.expires_at
    if exp_at.tzinfo is None:
        exp_at = exp_at.replace(tzinfo=timezone.utc)
    if exp_at <= now:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Bản quyền đã hết hạn để thực hiện đồng bộ."
        )

    dev_stmt = select(ActivatedDevice).where(
        ActivatedDevice.license_id == license_obj.id,
        ActivatedDevice.hwid == req.hwid,
        ActivatedDevice.current_session_token == req.session_token,
        ActivatedDevice.is_revoked == False
    )
    dev_res = await db.execute(dev_stmt)
    device_obj = dev_res.scalar_one_or_none()

    if not device_obj:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Xác thực thiết bị hoặc session thất bại."
        )

    # 2. Upsert posts and comments
    posts_synced = 0
    comments_synced = 0

    for post_item in req.posts:
        # Check existing post
        post_stmt = select(CloudPost).where(
            CloudPost.license_id == license_obj.id,
            CloudPost.post_id == post_item.post_id
        )
        post_res = await db.execute(post_stmt)
        post_record = post_res.scalar_one_or_none()

        if not post_record:
            post_record = CloudPost(
                license_id=license_obj.id,
                post_id=post_item.post_id,
                post_url=post_item.post_url,
                author_name=post_item.author_name,
                content_preview=post_item.content_preview,
                total_comments_crawled=post_item.total_comments_crawled,
                total_orders_detected=post_item.total_orders_detected,
                total_phones_detected=post_item.total_phones_detected,
                status=post_item.status,
                last_crawled_at=post_item.last_crawled_at
            )
            db.add(post_record)
            await db.flush()
        else:
            post_record.author_name = post_item.author_name or post_record.author_name
            post_record.content_preview = post_item.content_preview or post_record.content_preview
            post_record.total_comments_crawled = max(post_record.total_comments_crawled, post_item.total_comments_crawled)
            post_record.total_orders_detected = max(post_record.total_orders_detected, post_item.total_orders_detected)
            post_record.total_phones_detected = max(post_record.total_phones_detected, post_item.total_phones_detected)
            post_record.status = post_item.status
            post_record.last_crawled_at = post_item.last_crawled_at or post_record.last_crawled_at

        posts_synced += 1

        # Process comments
        for comment_item in (post_item.comments or []):
            cm_stmt = select(CloudComment).where(
                CloudComment.post_id == post_item.post_id,
                CloudComment.comment_id == comment_item.id
            )
            cm_res = await db.execute(cm_stmt)
            cm_record = cm_res.scalar_one_or_none()

            if not cm_record:
                cm_record = CloudComment(
                    post_table_id=post_record.id,
                    comment_id=comment_item.id,
                    post_id=post_item.post_id,
                    parent_comment_id=comment_item.parent_comment_id,
                    author_id=comment_item.author_id,
                    author_name=comment_item.author_name,
                    author_url=comment_item.author_url,
                    content=comment_item.content,
                    phone_numbers=comment_item.phone_numbers,
                    intent_tag=comment_item.intent_tag,
                    comment_created_at=comment_item.created_at
                )
                db.add(cm_record)
                comments_synced += 1

    await db.commit()

    return SyncBatchResponse(
        success=True,
        message="Đồng bộ dữ liệu lên cloud thành công.",
        posts_synced=posts_synced,
        comments_synced=comments_synced
    )
