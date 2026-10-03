import os
import json
import base64
from pathlib import Path
from typing import Dict, Any, Tuple
from cryptography.hazmat.primitives.asymmetric import ed25519
from cryptography.hazmat.primitives import serialization

KEYS_DIR = Path(__file__).parent.parent / "keys"
PRIVATE_KEY_PATH = KEYS_DIR / "ed25519_private.pem"
PUBLIC_KEY_PATH = KEYS_DIR / "ed25519_public.pem"


class LicenseSecurityManager:
    def __init__(
        self,
        private_key_path: Path = PRIVATE_KEY_PATH,
        public_key_path: Path = PUBLIC_KEY_PATH
    ):
        self.private_key_path = private_key_path
        self.public_key_path = public_key_path
        self._private_key: ed25519.Ed25519PrivateKey = None
        self._public_key: ed25519.Ed25519PublicKey = None
        self.ensure_keys()

    def ensure_keys(self):
        """Load existing Ed25519 keys or generate a new keypair if not found."""
        if self.private_key_path.exists() and self.public_key_path.exists():
            with open(self.private_key_path, "rb") as f:
                self._private_key = serialization.load_pem_private_key(
                    f.read(),
                    password=None
                )
            with open(self.public_key_path, "rb") as f:
                self._public_key = serialization.load_pem_public_key(
                    f.read()
                )
        else:
            self.generate_new_keypair()

    def generate_new_keypair(self) -> Tuple[str, str]:
        """Generate a new Ed25519 keypair and save to disk."""
        KEYS_DIR.mkdir(parents=True, exist_ok=True)
        self._private_key = ed25519.Ed25519PrivateKey.generate()
        self._public_key = self._private_key.public_key()

        private_bytes = self._private_key.private_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PrivateFormat.PKCS8,
            encryption_algorithm=serialization.NoEncryption()
        )
        public_bytes = self._public_key.public_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PublicFormat.SubjectPublicKeyInfo
        )

        with open(self.private_key_path, "wb") as f:
            f.write(private_bytes)
        with open(self.public_key_path, "wb") as f:
            f.write(public_bytes)

        return (
            self.get_public_key_raw_hex(),
            self.get_public_key_raw_base64()
        )

    def get_public_key_raw_bytes(self) -> bytes:
        """Returns the raw 32-byte Ed25519 public key."""
        return self._public_key.public_bytes(
            encoding=serialization.Encoding.Raw,
            format=serialization.PublicFormat.Raw
        )

    def get_public_key_raw_hex(self) -> str:
        """Returns the 32-byte public key in hex format for easy embedding in Rust code."""
        return self.get_public_key_raw_bytes().hex()

    def get_public_key_raw_base64(self) -> str:
        """Returns the 32-byte public key in base64 format."""
        return base64.b64encode(self.get_public_key_raw_bytes()).decode("utf-8")

    def sign_payload(self, payload: Dict[str, Any]) -> Dict[str, Any]:
        """
        Signs a dictionary payload using the Ed25519 private key.
        The payload is serialized deterministically with sorted keys.
        Returns:
            {
                "data": payload,
                "signature": base64_encoded_signature,
                "canonical_message": json_string_signed
            }
        """
        canonical_str = json.dumps(payload, sort_keys=True, separators=(',', ':'))
        canonical_bytes = canonical_str.encode("utf-8")
        signature = self._private_key.sign(canonical_bytes)
        sig_b64 = base64.b64encode(signature).decode("utf-8")

        return {
            "data": payload,
            "signature": sig_b64,
            "canonical_message": canonical_str
        }

    def verify_signature(self, canonical_message: str, signature_b64: str) -> bool:
        """Verifies an Ed25519 signature against the canonical message string."""
        try:
            sig_bytes = base64.b64decode(signature_b64)
            self._public_key.verify(sig_bytes, canonical_message.encode("utf-8"))
            return True
        except Exception:
            return False


# Singleton instance
security_manager = LicenseSecurityManager()
