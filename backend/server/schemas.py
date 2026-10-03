from typing import List, Optional, Dict, Any
from pydantic import BaseModel, Field

# --- License Schemas ---
class LoginRequest(BaseModel):
    license_key: str = Field(..., description="Unique license key", min_length=4)
    hwid: str = Field(..., description="SHA-256 hardware identifier", min_length=16)
    device_name: Optional[str] = Field(None, description="Human readable device name/OS")

class HeartbeatRequest(BaseModel):
    license_key: str = Field(..., description="Unique license key")
    hwid: str = Field(..., description="SHA-256 hardware identifier")
    session_token: str = Field(..., description="Current active session token")

class SignedPayload(BaseModel):
    data: Dict[str, Any]
    signature: str
    canonical_message: str

class LicenseResponse(BaseModel):
    success: bool
    message: str
    client_name: Optional[str] = None
    expires_at: Optional[str] = None
    session_token: Optional[str] = None
    signed_payload: SignedPayload

# --- Sync Schemas ---
class SyncCommentItem(BaseModel):
    id: str
    post_id: str
    parent_comment_id: Optional[str] = None
    author_id: Optional[str] = None
    author_name: Optional[str] = None
    author_url: Optional[str] = None
    content: Optional[str] = None
    phone_numbers: Optional[str] = None
    intent_tag: Optional[str] = None
    created_at: Optional[int] = None

class SyncPostItem(BaseModel):
    post_id: str
    post_url: Optional[str] = None
    author_name: Optional[str] = None
    content_preview: Optional[str] = None
    total_comments_crawled: int = 0
    total_orders_detected: int = 0
    total_phones_detected: int = 0
    status: str = "ACTIVE"
    last_crawled_at: Optional[int] = None
    comments: Optional[List[SyncCommentItem]] = []

class SyncBatchRequest(BaseModel):
    license_key: str
    hwid: str
    session_token: str
    posts: List[SyncPostItem]

class SyncBatchResponse(BaseModel):
    success: bool
    message: str
    posts_synced: int
    comments_synced: int
