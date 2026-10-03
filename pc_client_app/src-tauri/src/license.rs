use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use obfstr::obfstring;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

// Embedded Ed25519 Public Key generated from server (Hex 32-bytes)
// Protected with obfstr macro to prevent string extraction from binary
const DEFAULT_SERVER_URL: &str = "http://127.0.0.1:8000";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedPayload {
    pub data: serde_json::Value,
    pub signature: String,
    pub canonical_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseResponse {
    pub success: bool,
    pub message: String,
    pub client_name: Option<String>,
    pub expires_at: Option<String>,
    pub session_token: Option<String>,
    pub signed_payload: SignedPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseStateInfo {
    pub is_authenticated: bool,
    pub license_key: Option<String>,
    pub client_name: Option<String>,
    pub expires_at: Option<String>,
    pub hwid: String,
    pub server_url: String,
}

pub struct LicenseManager {
    pub is_authenticated: Arc<AtomicBool>,
    pub consecutive_failures: Arc<AtomicU32>,
    pub current_session_token: Arc<RwLock<Option<String>>>,
    pub license_key: Arc<RwLock<Option<String>>>,
    pub client_name: Arc<RwLock<Option<String>>>,
    pub expires_at: Arc<RwLock<Option<String>>>,
    pub hwid: Arc<RwLock<String>>,
    pub server_url: Arc<RwLock<String>>,
}

impl LicenseManager {
    pub fn new() -> Self {
        let hwid = crate::hwid::get_hardware_id();
        Self {
            is_authenticated: Arc::new(AtomicBool::new(false)),
            consecutive_failures: Arc::new(AtomicU32::new(0)),
            current_session_token: Arc::new(RwLock::new(None)),
            license_key: Arc::new(RwLock::new(None)),
            client_name: Arc::new(RwLock::new(None)),
            expires_at: Arc::new(RwLock::new(None)),
            hwid: Arc::new(RwLock::new(hwid)),
            server_url: Arc::new(RwLock::new(DEFAULT_SERVER_URL.to_string())),
        }
    }

    pub fn get_embedded_public_key_bytes() -> Result<[u8; 32], String> {
        let pubkey_hex = obfstring!("72bd488758006951e5db3d22bb28badd1a5350b29b204101a4c770a97c51ec36");
        let bytes = hex::decode(&pubkey_hex).map_err(|e| format!("Decode hex error: {}", e))?;
        if bytes.len() != 32 {
            return Err("Invalid public key length".to_string());
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(arr)
    }

    pub fn verify_signature(canonical_message: &str, signature_b64: &str) -> Result<bool, String> {
        let pubkey_bytes = Self::get_embedded_public_key_bytes()?;
        let verifying_key = VerifyingKey::from_bytes(&pubkey_bytes)
            .map_err(|e| format!("Invalid VerifyingKey: {}", e))?;

        let sig_bytes = base64::engine::general_purpose::STANDARD
            .decode(signature_b64)
            .map_err(|e| format!("Invalid base64 signature: {}", e))?;

        if sig_bytes.len() != 64 {
            return Err("Signature bytes must be 64 bytes".to_string());
        }

        let mut sig_arr = [0u8; 64];
        sig_arr.copy_from_slice(&sig_bytes);
        let signature = Signature::from_bytes(&sig_arr);

        verifying_key
            .verify(canonical_message.as_bytes(), &signature)
            .map_err(|e| format!("Signature verification failed: {}", e))?;

        Ok(true)
    }

    pub async fn login(
        &self,
        license_key: &str,
        custom_server_url: Option<String>,
    ) -> Result<LicenseResponse, String> {
        if let Some(url) = custom_server_url {
            let mut s = self.server_url.write().await;
            *s = url;
        }

        let base_url = self.server_url.read().await.clone();
        let login_endpoint = format!("{}/api/v1/license/login", base_url);
        let hwid = self.hwid.read().await.clone();
        let device_name = crate::hwid::get_device_name();

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let body = serde_json::json!({
            "license_key": license_key.trim(),
            "hwid": hwid,
            "device_name": device_name
        });

        let resp = client
            .post(&login_endpoint)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Lỗi kết nối Server xác thực (Online-only): {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let err_txt = resp.text().await.unwrap_or_default();
            self.is_authenticated.store(false, Ordering::SeqCst);
            return Err(format!("Xác thực thất bại [{}]: {}", status.as_u16(), err_txt));
        }

        let lic_resp: LicenseResponse = resp
            .json()
            .await
            .map_err(|e| format!("Lỗi giải mã phản hồi server: {}", e))?;

        // Verify cryptographic signature from server
        Self::verify_signature(
            &lic_resp.signed_payload.canonical_message,
            &lic_resp.signed_payload.signature,
        )?;

        // Verify payload contents
        if let Some(token) = &lic_resp.session_token {
            let mut token_lock = self.current_session_token.write().await;
            *token_lock = Some(token.clone());
        }

        {
            let mut key_lock = self.license_key.write().await;
            *key_lock = Some(license_key.trim().to_string());
        }
        {
            let mut name_lock = self.client_name.write().await;
            *name_lock = lic_resp.client_name.clone();
        }
        {
            let mut exp_lock = self.expires_at.write().await;
            *exp_lock = lic_resp.expires_at.clone();
        }

        self.consecutive_failures.store(0, Ordering::SeqCst);
        self.is_authenticated.store(true, Ordering::SeqCst);

        Ok(lic_resp)
    }

    pub async fn send_heartbeat(&self) -> Result<LicenseResponse, String> {
        let is_auth = self.is_authenticated.load(Ordering::SeqCst);
        if !is_auth {
            return Err("Chưa đăng nhập bản quyền.".to_string());
        }

        let license_key = self.license_key.read().await.clone()
            .ok_or_else(|| "Thiếu license key.".to_string())?;
        let session_token = self.current_session_token.read().await.clone()
            .ok_or_else(|| "Thiếu session token.".to_string())?;
        let hwid = self.hwid.read().await.clone();
        let base_url = self.server_url.read().await.clone();
        let heartbeat_endpoint = format!("{}/api/v1/license/heartbeat", base_url);

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("HTTP client error: {}", e))?;

        let body = serde_json::json!({
            "license_key": license_key,
            "hwid": hwid,
            "session_token": session_token
        });

        let resp = client
            .post(&heartbeat_endpoint)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Mất kết nối mạng hoặc server không phản hồi: {}", e))?;

        if resp.status().as_u16() == 403 {
            self.lock_application().await;
            return Err("Bản quyền bị khóa hoặc hết hạn từ phía máy chủ (403 Forbidden).".to_string());
        }

        if !resp.status().is_success() {
            return Err(format!("Lỗi heartbeat: HTTP {}", resp.status()));
        }

        let lic_resp: LicenseResponse = resp
            .json()
            .await
            .map_err(|e| format!("Giải mã phản hồi heartbeat thất bại: {}", e))?;

        Self::verify_signature(
            &lic_resp.signed_payload.canonical_message,
            &lic_resp.signed_payload.signature,
        )?;

        // Reset failure counter on success
        self.consecutive_failures.store(0, Ordering::SeqCst);

        if let Some(exp) = &lic_resp.expires_at {
            let mut exp_lock = self.expires_at.write().await;
            *exp_lock = Some(exp.clone());
        }

        Ok(lic_resp)
    }

    pub async fn lock_application(&self) {
        self.is_authenticated.store(false, Ordering::SeqCst);
        let mut token = self.current_session_token.write().await;
        *token = None;
    }

    pub async fn get_status_info(&self) -> LicenseStateInfo {
        LicenseStateInfo {
            is_authenticated: self.is_authenticated.load(Ordering::SeqCst),
            license_key: self.license_key.read().await.clone(),
            client_name: self.client_name.read().await.clone(),
            expires_at: self.expires_at.read().await.clone(),
            hwid: self.hwid.read().await.clone(),
            server_url: self.server_url.read().await.clone(),
        }
    }
}

/// Spawns the background heartbeat loop: runs every 5 minutes (300 seconds).
/// If heartbeat returns 403 or network failure persists for > 2 cycles (10 min),
/// locks the app and emits `license_locked` event.
pub fn start_heartbeat_worker(app_handle: AppHandle, manager: Arc<LicenseManager>) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;

            if !manager.is_authenticated.load(Ordering::SeqCst) {
                continue;
            }

            match manager.send_heartbeat().await {
                Ok(resp) => {
                    let _ = app_handle.emit("license_heartbeat_ok", &resp);
                }
                Err(err) => {
                    let fails = manager.consecutive_failures.fetch_add(1, Ordering::SeqCst) + 1;
                    eprintln!("[License Heartbeat Warning] Lần {}: {}", fails, err);

                    let _ = app_handle.emit("license_heartbeat_warning", serde_json::json!({
                        "failures": fails,
                        "error": err
                    }));

                    // Strict online-only rule: if >= 2 cycles failure, lock immediately
                    if fails >= 2 {
                        eprintln!("[License CRITICAL] Mất kết nối quá 2 chu kỳ (10 phút) hoặc bản quyền bị thu hồi. Đang khóa ứng dụng!");
                        manager.lock_application().await;
                        let _ = app_handle.emit("license_locked", serde_json::json!({
                            "reason": "Mất kết nối kiểm tra bản quyền trực tuyến quá 10 phút hoặc bản quyền đã bị thu hồi."
                        }));
                    }
                }
            }
        }
    });
}
