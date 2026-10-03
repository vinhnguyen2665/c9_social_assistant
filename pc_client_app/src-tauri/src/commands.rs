use chrono::Utc;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::crawler::{FacebookCrawler, FacebookSession};
use crate::db::{CommentItem, Database, MonitoredPost};
use crate::license::{LicenseManager, LicenseResponse, LicenseStateInfo};
use crate::tracker::WatchlistTracker;

pub struct AppState {
    pub db: Arc<Database>,
    pub crawler: Arc<FacebookCrawler>,
    pub license: Arc<LicenseManager>,
    pub tracker: Arc<WatchlistTracker>,
}

#[tauri::command]
pub async fn login_license(
    state: State<'_, AppState>,
    license_key: String,
    server_url: Option<String>,
) -> Result<LicenseResponse, String> {
    state.license.login(&license_key, server_url).await
}

#[tauri::command]
pub async fn get_license_status(state: State<'_, AppState>) -> Result<LicenseStateInfo, String> {
    Ok(state.license.get_status_info().await)
}

#[tauri::command]
pub async fn add_monitored_post(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    url_or_id: String,
    crawl_interval_minutes: Option<i64>,
) -> Result<MonitoredPost, String> {
    let post_id = FacebookCrawler::extract_post_id(&url_or_id);
    if post_id.is_empty() {
        return Err("Không thể bóc tách Post ID từ đường dẫn đã nhập.".to_string());
    }

    let interval = crawl_interval_minutes.unwrap_or(30).max(1);
    let now = Utc::now().timestamp();
    let post_url = if url_or_id.starts_with("http") {
        url_or_id.clone()
    } else {
        format!("https://www.facebook.com/{}", post_id)
    };

    let mut author_name = Some("Bài viết Facebook".to_string());
    let mut content_preview = Some("Đang quét nội dung ban đầu...".to_string());
    let mut initial_comments = Vec::new();
    let mut resolved_id: Option<String> = None;

    // Fetch initial post details (author, preview, embedded comments)
    if let Ok(details) = state.crawler.fetch_post_details(&post_url, &post_id).await {
        if let Some(author) = details.author_name {
            author_name = Some(author);
        }
        if let Some(preview) = details.content_preview {
            content_preview = Some(preview);
        }
        if let Some(dtsg) = details.fb_dtsg {
            if let Some(mut session) = state.crawler.get_session().await {
                if session.fb_dtsg == "NA" || session.fb_dtsg.is_empty() {
                    session.fb_dtsg = dtsg;
                    let _ = state.crawler.set_session(session.clone()).await;
                    if let Ok(j) = serde_json::to_string(&session) {
                        let _ = state.db.set_setting("fb_session", &j);
                    }
                }
            }
        }
        resolved_id = details.resolved_id;
        initial_comments = details.initial_comments;
    }

    let post = MonitoredPost {
        post_id: post_id.clone(),
        post_url,
        author_name,
        content_preview,
        total_comments_crawled: initial_comments.len() as i64,
        total_orders_detected: 0,
        total_phones_detected: 0,
        status: "ACTIVE".to_string(),
        crawl_interval_minutes: interval,
        last_cursor: None,
        // Set to 0 so background worker crawls immediately on next tick
        last_crawled_at: Some(0),
        created_at: now,
    };

    state.db.add_or_update_post(&post)
        .map_err(|e| format!("Lỗi thêm bài viết: {}", e))?;

    if !initial_comments.is_empty() {
        let _ = state.db.batch_insert_comments(&initial_comments);
        let _ = state.db.update_post_statistics(&post_id, None);
    }

    // Trigger initial crawl so that newly added post immediately has all comments and sub-comments
    let tracker_clone = state.tracker.clone();
    let handle_clone = app_handle.clone();
    let post_id_clone = post_id.clone();
    let _ = resolved_id;

    let crawl_res = tracker_clone.crawl_post_incremental(&post_id_clone, Some(&handle_clone)).await;
    match crawl_res {
        Ok(count) => {
            let _ = handle_clone.emit("post_initial_crawl_done", serde_json::json!({
                "post_id": post_id_clone,
                "new_comments": count
            }));
        }
        Err(e) => {
            eprintln!("[AddPost] Lỗi crawl ban đầu cho {}: {}", post_id_clone, e);
        }
    }

    if let Ok(Some(fresh)) = state.db.get_post(&post_id) {
        return Ok(fresh);
    }

    Ok(post)
}

#[tauri::command]
pub async fn get_monitored_posts(state: State<'_, AppState>) -> Result<Vec<MonitoredPost>, String> {
    state.db.get_monitored_posts().map_err(|e| format!("Lỗi đọc DB: {}", e))
}

#[tauri::command]
pub async fn toggle_post_status(
    state: State<'_, AppState>,
    post_id: String,
    status: String,
) -> Result<bool, String> {
    state.db.update_post_status(&post_id, &status)
        .map_err(|e| format!("Lỗi cập nhật trạng thái: {}", e))?;
    Ok(true)
}

#[tauri::command]
pub async fn update_monitored_post(
    state: State<'_, AppState>,
    post_id: String,
    author_name: Option<String>,
    content_preview: Option<String>,
    crawl_interval_minutes: Option<i64>,
    status: Option<String>,
) -> Result<MonitoredPost, String> {
    state.db.update_monitored_post(
        &post_id,
        author_name.as_deref(),
        content_preview.as_deref(),
        crawl_interval_minutes,
        status.as_deref(),
    ).map_err(|e| format!("Lỗi cập nhật bài viết: {}", e))?;

    state.db.get_post(&post_id)
        .map_err(|e| format!("Lỗi đọc DB: {}", e))?
        .ok_or_else(|| "Không tìm thấy bài viết".to_string())
}

#[tauri::command]
pub async fn delete_monitored_post(
    state: State<'_, AppState>,
    post_id: String,
) -> Result<bool, String> {
    state.db.delete_post(&post_id)
        .map_err(|e| format!("Lỗi xoá bài viết: {}", e))?;
    Ok(true)
}

#[tauri::command]
pub async fn scan_post_now(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    post_id: String,
) -> Result<usize, String> {
    state.tracker.crawl_post_incremental(&post_id, Some(&app_handle)).await
}

#[tauri::command]
pub async fn get_post_comments(
    state: State<'_, AppState>,
    post_id: String,
    intent_filter: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<CommentItem>, String> {
    state.db.get_comments(
        &post_id,
        intent_filter.as_deref(),
        limit.unwrap_or(100),
        offset.unwrap_or(0),
    ).map_err(|e| format!("Lỗi lấy danh sách comment: {}", e))
}

#[tauri::command]
pub async fn open_facebook_login_window(app_handle: AppHandle) -> Result<bool, String> {
    if let Some(w) = app_handle.get_webview_window("fb_login") {
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(true);
    }

    let url: tauri::Url = "https://www.facebook.com/login".parse()
        .map_err(|e| format!("URL Error: {}", e))?;

    WebviewWindowBuilder::new(&app_handle, "fb_login", WebviewUrl::External(url))
        .title("Đăng nhập Facebook (Hỗ trợ 2FA) - C9 Social Assistant")
        .inner_size(580.0, 750.0)
        .resizable(true)
        .build()
        .map_err(|e| format!("Không thể mở cửa sổ Facebook: {}", e))?;

    Ok(true)
}

#[tauri::command]
pub async fn extract_facebook_session_from_window(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let window = app_handle.get_webview_window("fb_login")
        .ok_or_else(|| "Cửa sổ Facebook chưa được mở. Hãy bấm 'Mở Cửa Sổ Đăng Nhập Facebook' trước.".to_string())?;

    let mut c_user = String::new();
    let mut xs = String::new();
    let mut datr = None;
    let mut fr = None;

    // 1. Extract directly from Native Browser CookieStore (handles HttpOnly cookies like c_user, xs, datr, fr)
    if let Ok(cookie_list) = window.cookies() {
        for cookie in cookie_list {
            let name = cookie.name();
            let val = cookie.value();
            if name == "c_user" {
                c_user = val.trim().to_string();
            } else if name == "xs" {
                xs = val.trim().to_string();
            } else if name == "datr" {
                datr = Some(val.trim().to_string());
            } else if name == "fr" {
                fr = Some(val.trim().to_string());
            }
        }
    }

    // 2. Extract fb_dtsg and fallback cookies from page DOM via JavaScript
    let js_code = r#"
        (function() {
            var cookies = document.cookie || '';
            var js_c_user = (cookies.match(/c_user=([^;]+)/) || [])[1] || '';
            var js_xs = (cookies.match(/xs=([^;]+)/) || [])[1] || '';

            var fb_dtsg = '';
            var dtsgInput = document.querySelector('input[name="fb_dtsg"]');
            if (dtsgInput && dtsgInput.value) {
                fb_dtsg = dtsgInput.value;
            } else {
                var html = document.documentElement.innerHTML;
                var m = html.match(/"DTSGInitialData",\[\],\{"token":"([^"]+)"/);
                if (m && m[1]) {
                    fb_dtsg = m[1];
                } else {
                    var m2 = html.match(/name="fb_dtsg"\s+value="([^"]+)"/);
                    if (m2 && m2[1]) fb_dtsg = m2[1];
                }
            }

            return JSON.stringify({
                c_user: js_c_user,
                xs: js_xs,
                fb_dtsg: fb_dtsg
            });
        })()
    "#;

    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    let tx_cell = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));

    window.eval_with_callback(js_code, move |result| {
        if let Ok(mut lock) = tx_cell.lock() {
            if let Some(sender) = lock.take() {
                let _ = sender.send(result);
            }
        }
    }).map_err(|e| format!("Lỗi gọi JavaScript trên WebView: {}", e))?;

    let mut fb_dtsg = "NA".to_string();
    if let Ok(json_str) = rx.await {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
            let actual_obj: serde_json::Value = if val.is_string() {
                serde_json::from_str(val.as_str().unwrap()).unwrap_or(val)
            } else {
                val
            };

            if c_user.is_empty() {
                c_user = actual_obj.get("c_user").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
            }
            if xs.is_empty() {
                xs = actual_obj.get("xs").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
            }
            if let Some(dtsg) = actual_obj.get("fb_dtsg").and_then(|v| v.as_str()) {
                if !dtsg.is_empty() {
                    fb_dtsg = dtsg.to_string();
                }
            }
        }
    }

    if c_user.is_empty() || xs.is_empty() {
        return Err("Chưa phát hiện tài khoản đăng nhập thành công (thiếu c_user hoặc xs trong CookieStore). Vui lòng hoàn tất đăng nhập và 2FA trên cửa sổ Facebook trước khi bấm trích xuất.".to_string());
    }

    let session = FacebookSession {
        c_user: c_user.clone(),
        xs,
        datr,
        fr,
        fb_dtsg,
    };

    state.crawler.set_session(session.clone()).await;

    // Persist session to SQLite so it survives restarts
    if let Ok(json_str) = serde_json::to_string(&session) {
        let _ = state.db.set_setting("fb_session", &json_str);
    }

    // Close the login window once successfully extracted
    let _ = window.close();

    Ok(format!("Trích xuất thành công tài khoản Facebook UID: {}", c_user))
}

#[tauri::command]
pub async fn set_facebook_session(
    state: State<'_, AppState>,
    c_user: String,
    xs: String,
    datr: Option<String>,
    fr: Option<String>,
    fb_dtsg: String,
) -> Result<bool, String> {
    let session = FacebookSession {
        c_user,
        xs,
        datr,
        fr,
        fb_dtsg,
    };
    state.crawler.set_session(session.clone()).await;
    if let Ok(json_str) = serde_json::to_string(&session) {
        let _ = state.db.set_setting("fb_session", &json_str);
    }
    Ok(true)
}

#[tauri::command]
pub async fn get_facebook_session_status(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.crawler.has_session().await)
}

#[tauri::command]
pub async fn sync_all_to_cloud(state: State<'_, AppState>) -> Result<String, String> {
    let is_auth = state.license.is_authenticated.load(std::sync::atomic::Ordering::SeqCst);
    if !is_auth {
        return Err("Bản quyền chưa được xác thực trực tuyến.".to_string());
    }

    let license_key = state.license.license_key.read().await.clone()
        .ok_or_else(|| "Thiếu license key".to_string())?;
    let session_token = state.license.current_session_token.read().await.clone()
        .ok_or_else(|| "Thiếu session token".to_string())?;
    let hwid = state.license.hwid.read().await.clone();
    let server_url = state.license.server_url.read().await.clone();
    let sync_url = format!("{}/api/v1/sync/batch", server_url);

    let posts = state.db.get_monitored_posts()
        .map_err(|e| format!("Lỗi đọc DB: {}", e))?;

    let mut sync_posts = Vec::new();

    for p in posts {
        let comments = state.db.get_comments(&p.post_id, None, 200, 0).unwrap_or_default();
        sync_posts.push(serde_json::json!({
            "post_id": p.post_id,
            "post_url": p.post_url,
            "author_name": p.author_name,
            "content_preview": p.content_preview,
            "total_comments_crawled": p.total_comments_crawled,
            "total_orders_detected": p.total_orders_detected,
            "total_phones_detected": p.total_phones_detected,
            "status": p.status,
            "last_crawled_at": p.last_crawled_at,
            "comments": comments
        }));
    }

    let payload = serde_json::json!({
        "license_key": license_key,
        "hwid": hwid,
        "session_token": session_token,
        "posts": sync_posts
    });

    let client = reqwest::Client::new();
    let resp = client.post(&sync_url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Lỗi gửi đồng bộ lên Cloud: {}", e))?;

    if !resp.status().is_success() {
        let err = resp.text().await.unwrap_or_default();
        return Err(format!("Lỗi server Cloud: {}", err));
    }

    let val: serde_json::Value = resp.json().await.map_err(|e| format!("Parse json err: {}", e))?;
    Ok(val.get("message").and_then(|m| m.as_str()).unwrap_or("Đồng bộ thành công!").to_string())
}

#[tauri::command]
pub async fn seed_demo_data(state: State<'_, AppState>) -> Result<bool, String> {
    let now = Utc::now().timestamp();
    let demo_post = MonitoredPost {
        post_id: "1098234710928374".to_string(),
        post_url: "https://www.facebook.com/watch/?v=1098234710928374".to_string(),
        author_name: Some("Thời Trang Thiết Kế C9 Store".to_string()),
        content_preview: Some("Xả kho hè toàn bộ đầm hoa nhí cao cấp chỉ từ 199k! Miễn phí giao hàng khi đặt 2 áo trở lên...".to_string()),
        total_comments_crawled: 0,
        total_orders_detected: 0,
        total_phones_detected: 0,
        status: "ACTIVE".to_string(),
        crawl_interval_minutes: 15,
        last_cursor: None,
        last_crawled_at: Some(now - 300),
        created_at: now - 3600,
    };

    state.db.add_or_update_post(&demo_post)
        .map_err(|e| format!("DB Err: {}", e))?;

    let demo_comments = vec![
        CommentItem {
            id: "fb_cm_101".to_string(),
            post_id: demo_post.post_id.clone(),
            parent_comment_id: None,
            author_id: Some("100012345678".to_string()),
            author_name: Some("Thanh Huyền".to_string()),
            author_url: Some("https://facebook.com/thanhhuyen".to_string()),
            content: Some("Chốt 2 váy màu be size M, ship về 45 Lê Duẩn, Đà Nẵng nha shop. Sđt: 0912.456.789".to_string()),
            phone_numbers: Some("0912456789".to_string()),
            intent_tag: Some("[Chốt đơn]".to_string()),
            created_at: Some(now - 2400),
        },
        CommentItem {
            id: "fb_cm_102".to_string(),
            post_id: demo_post.post_id.clone(),
            parent_comment_id: None,
            author_id: Some("100087654321".to_string()),
            author_name: Some("Ngọc Mai".to_string()),
            author_url: Some("https://facebook.com/ngocmai".to_string()),
            content: Some("Váy hoa nhí giá bao nhiêu 1 chiếc vậy shop ơi? Có được kiểm tra hàng trước không?".to_string()),
            phone_numbers: None,
            intent_tag: Some("[Hỏi giá]".to_string()),
            created_at: Some(now - 1800),
        },
        CommentItem {
            id: "fb_cm_103".to_string(),
            post_id: demo_post.post_id.clone(),
            parent_comment_id: None,
            author_id: Some("100045612378".to_string()),
            author_name: Some("Bảo Trân".to_string()),
            author_url: Some("https://facebook.com/baotran".to_string()),
            content: Some("Lấy 1 chiếc size L màu xanh, gọi mình số 0868 999 888 nhé!".to_string()),
            phone_numbers: Some("0868999888".to_string()),
            intent_tag: Some("[Chốt đơn]".to_string()),
            created_at: Some(now - 1200),
        },
        CommentItem {
            id: "fb_cm_104".to_string(),
            post_id: demo_post.post_id.clone(),
            parent_comment_id: None,
            author_id: Some("100099887766".to_string()),
            author_name: Some("Quang Tuấn".to_string()),
            author_url: Some("https://facebook.com/quangtuan".to_string()),
            content: Some("Shop làm ăn kiểu gì mà hôm qua gửi thiếu quà tặng của mình thế? Đổi hàng giúp mình".to_string()),
            phone_numbers: None,
            intent_tag: Some("[Khiếu nại]".to_string()),
            created_at: Some(now - 600),
        },
        CommentItem {
            id: "fb_cm_105".to_string(),
            post_id: demo_post.post_id.clone(),
            parent_comment_id: None,
            author_id: Some("100077889900".to_string()),
            author_name: Some("Trang Kiếm Tiền".to_string()),
            author_url: Some("https://facebook.com/trangkiemtien".to_string()),
            content: Some("Cơ hội kiếm thêm thu nhập 500k/ngày tại nhà, vào zalo.me/g/demo để nhận việc ngay".to_string()),
            phone_numbers: None,
            intent_tag: Some("[Spam]".to_string()),
            created_at: Some(now - 100),
        },
    ];

    state.db.batch_insert_comments(&demo_comments)
        .map_err(|e| format!("Lỗi insert demo comments: {}", e))?;

    let _ = state.db.update_post_statistics(&demo_post.post_id, None);

    Ok(true)
}
