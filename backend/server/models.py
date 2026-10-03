import uuid
from datetime import datetime, timezone
from sqlalchemy import (
    Column,
    String,
    Integer,
    Boolean,
    DateTime,
    ForeignKey,
    Text,
    BigInteger,
    Index
)
from sqlalchemy.dialects.postgresql import UUID as PG_UUID
from sqlalchemy.types import TypeDecorator, CHAR
from sqlalchemy.orm import relationship
from .database import Base

def utc_now():
    return datetime.now(timezone.utc)

class GUID(TypeDecorator):
    """Platform-independent GUID/UUID type.
    Uses PostgreSQL's UUID type, otherwise uses CHAR(36).
    """
    impl = CHAR
    cache_ok = True

    def load_dialect_impl(self, dialect):
        if dialect.name == "postgresql":
            return dialect.type_descriptor(PG_UUID(as_uuid=True))
        else:
            return dialect.type_descriptor(CHAR(36))

    def process_bind_param(self, value, dialect):
        if value is None:
            return value
        elif dialect.name == "postgresql":
            return str(value)
        else:
            if isinstance(value, uuid.UUID):
                return str(value)
            return str(uuid.UUID(str(value)))

    def process_result_value(self, value, dialect):
        if value is None:
            return value
        if not isinstance(value, uuid.UUID):
            return uuid.UUID(str(value))
        return value


class License(Base):
    __tablename__ = "licenses"

    id = Column(GUID, primary_key=True, default=uuid.uuid4)
    license_key = Column(String(128), unique=True, nullable=False, index=True)
    client_name = Column(String(255), nullable=False)
    max_devices = Column(Integer, default=1, nullable=False)
    expires_at = Column(DateTime(timezone=True), nullable=False)
    is_active = Column(Boolean, default=True, nullable=False)
    created_at = Column(DateTime(timezone=True), default=utc_now, nullable=False)

    devices = relationship("ActivatedDevice", back_populates="license", cascade="all, delete-orphan")
    cloud_posts = relationship("CloudPost", back_populates="license", cascade="all, delete-orphan")


class ActivatedDevice(Base):
    __tablename__ = "activated_devices"

    id = Column(GUID, primary_key=True, default=uuid.uuid4)
    license_id = Column(GUID, ForeignKey("licenses.id", ondelete="CASCADE"), nullable=False, index=True)
    hwid = Column(String(128), nullable=False, index=True)  # SHA256 of hardware
    device_name = Column(String(255), nullable=True)
    current_session_token = Column(String(255), nullable=True)
    last_heartbeat = Column(DateTime(timezone=True), default=utc_now, nullable=False)
    is_revoked = Column(Boolean, default=False, nullable=False)

    license = relationship("License", back_populates="devices")

    __table_args__ = (
        Index("idx_license_hwid", "license_id", "hwid", unique=True),
    )


class CloudPost(Base):
    __tablename__ = "cloud_posts"

    id = Column(GUID, primary_key=True, default=uuid.uuid4)
    license_id = Column(GUID, ForeignKey("licenses.id", ondelete="CASCADE"), nullable=False, index=True)
    post_id = Column(String(128), nullable=False, index=True)
    post_url = Column(Text, nullable=True)
    author_name = Column(String(255), nullable=True)
    content_preview = Column(Text, nullable=True)
    total_comments_crawled = Column(Integer, default=0, nullable=False)
    total_orders_detected = Column(Integer, default=0, nullable=False)
    total_phones_detected = Column(Integer, default=0, nullable=False)
    status = Column(String(50), default="ACTIVE", nullable=False)
    last_crawled_at = Column(BigInteger, nullable=True)
    created_at = Column(DateTime(timezone=True), default=utc_now, nullable=False)

    license = relationship("License", back_populates="cloud_posts")
    comments = relationship("CloudComment", back_populates="post", cascade="all, delete-orphan")

    __table_args__ = (
        Index("idx_license_post", "license_id", "post_id", unique=True),
    )


class CloudComment(Base):
    __tablename__ = "cloud_comments"

    id = Column(GUID, primary_key=True, default=uuid.uuid4)
    post_table_id = Column(GUID, ForeignKey("cloud_posts.id", ondelete="CASCADE"), nullable=False, index=True)
    comment_id = Column(String(128), nullable=False, index=True)
    post_id = Column(String(128), nullable=False, index=True)
    parent_comment_id = Column(String(128), nullable=True)
    author_id = Column(String(128), nullable=True)
    author_name = Column(String(255), nullable=True)
    author_url = Column(Text, nullable=True)
    content = Column(Text, nullable=True)
    phone_numbers = Column(Text, nullable=True)
    intent_tag = Column(String(50), nullable=True)
    comment_created_at = Column(BigInteger, nullable=True)
    synced_at = Column(DateTime(timezone=True), default=utc_now, nullable=False)

    post = relationship("CloudPost", back_populates="comments")

    __table_args__ = (
        Index("idx_cloud_post_comment", "post_id", "comment_id", unique=True),
    )
