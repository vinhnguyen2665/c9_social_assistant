use base64::prelude::*;
use chrono::Utc;
use obfstr::obfstring;
use rand::Rng;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::db::CommentItem;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacebookSession {
    pub c_user: String,
    pub xs: String,
    pub datr: Option<String>,
    pub fr: Option<String>,
    pub fb_dtsg: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentReplyTarget {
    pub parent_id: String,
    pub feedback_id: String,
    pub expansion_token: String,
    pub replies_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlPageResult {
    pub comments: Vec<CommentItem>,
    pub next_cursor: Option<String>,
    pub has_next_page: bool,
    pub is_checkpoint: bool,
    pub is_rate_limited: bool,
    /// Total comment count reported by Facebook for this post (if available)
    pub total_count: Option<i64>,
    pub reply_targets: Vec<CommentReplyTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostDetails {
    pub resolved_id: Option<String>,
    pub author_name: Option<String>,
    pub content_preview: Option<String>,
    pub initial_comments: Vec<CommentItem>,
    pub fb_dtsg: Option<String>,
    /// Total comment count embedded in the post page HTML (if available)
    pub total_comment_count: Option<i64>,
    pub reply_targets: Vec<CommentReplyTarget>,
}

fn decode_entities(input: &str) -> String {
    input
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&apos;", "'")
        .replace("&#x27;", "'")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("\\n", "\n")
        .replace("\\\"", "\"")
}

pub struct FacebookCrawler {
    session: Arc<RwLock<Option<FacebookSession>>>,
    client: reqwest::Client,
}

impl FacebookCrawler {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            session: Arc::new(RwLock::new(None)),
            client,
        }
    }

    pub async fn set_session(&self, session: FacebookSession) {
        let mut s = self.session.write().await;
        *s = Some(session);
    }

    pub async fn get_session(&self) -> Option<FacebookSession> {
        self.session.read().await.clone()
    }

    pub async fn has_session(&self) -> bool {
        self.session.read().await.is_some()
    }

    /// Normalizes different Facebook post URLs into a clean post ID
    pub fn extract_post_id(url_or_id: &str) -> String {
        let input = url_or_id.trim();

        // If it does not contain slashes, it's already an ID
        if !input.contains('/') && !input.contains('?') && !input.is_empty() {
            return input.to_string();
        }

        // Regex for /posts/([a-zA-Z0-9_]+)
        if let Ok(re) = Regex::new(r"/posts/([a-zA-Z0-9_]+)") {
            if let Some(caps) = re.captures(input) {
                if let Some(m) = caps.get(1) {
                    return m.as_str().to_string();
                }
            }
        }

        // Regex for mobile share links: /share/([a-zA-Z0-9_]+) or /share/(p|v|r)/([a-zA-Z0-9_]+)
        if let Ok(re) = Regex::new(r"/share/(?:(?:p|v|r)/)?([a-zA-Z0-9_]+)") {
            if let Some(caps) = re.captures(input) {
                if let Some(m) = caps.get(1) {
                    return m.as_str().to_string();
                }
            }
        }

        // Regex for story_fbid=([a-zA-Z0-9_]+)
        if let Ok(re) = Regex::new(r"story_fbid=([a-zA-Z0-9_]+)") {
            if let Some(caps) = re.captures(input) {
                if let Some(m) = caps.get(1) {
                    return m.as_str().to_string();
                }
            }
        }

        // Regex for fbid=([a-zA-Z0-9_]+)
        if let Ok(re) = Regex::new(r"[?&]fbid=([a-zA-Z0-9_]+)") {
            if let Some(caps) = re.captures(input) {
                if let Some(m) = caps.get(1) {
                    return m.as_str().to_string();
                }
            }
        }

        // Regex for /videos/([a-zA-Z0-9_]+) or /reel/([a-zA-Z0-9_]+)
        if let Ok(re) = Regex::new(r"/(?:videos|reel)/([a-zA-Z0-9_]+)") {
            if let Some(caps) = re.captures(input) {
                if let Some(m) = caps.get(1) {
                    return m.as_str().to_string();
                }
            }
        }

        // Fallback: extract last URL segment
        let trimmed_url = input.split('?').next().unwrap_or(input).trim_end_matches('/');
        if let Some(last_seg) = trimmed_url.split('/').last() {
            if !last_seg.is_empty() && last_seg != "posts" && !last_seg.starts_with("http") {
                return last_seg.to_string();
            }
        }

        input.to_string()
    }

    /// Fetches post HTML using current session cookies to extract author, content preview,
    /// resolved feedbackTargetID, fb_dtsg, and initial embedded comments.
    pub async fn fetch_post_details(&self, post_url: &str, post_id: &str) -> Result<PostDetails, String> {
        let session_opt = self.session.read().await.clone();

        let mut headers = HeaderMap::new();
        let user_agent = obfstring!("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36");
        if let Ok(val) = HeaderValue::from_str(&user_agent) {
            headers.insert(USER_AGENT, val);
        }
        headers.insert("accept", HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8"));
        headers.insert("accept-language", HeaderValue::from_static("vi-VN,vi;q=0.9,en-US;q=0.8,en;q=0.7"));
        headers.insert("sec-ch-ua", HeaderValue::from_static("\"Chromium\";v=\"124\", \"Google Chrome\";v=\"124\", \"Not-A.Brand\";v=\"99\""));
        headers.insert("sec-ch-ua-mobile", HeaderValue::from_static("?0"));
        headers.insert("sec-ch-ua-platform", HeaderValue::from_static("\"macOS\""));
        headers.insert("sec-fetch-dest", HeaderValue::from_static("document"));
        headers.insert("sec-fetch-mode", HeaderValue::from_static("navigate"));
        headers.insert("sec-fetch-site", HeaderValue::from_static("none"));
        headers.insert("sec-fetch-user", HeaderValue::from_static("?1"));
        headers.insert("upgrade-insecure-requests", HeaderValue::from_static("1"));

        if let Some(session) = &session_opt {
            let cookie_str = format!(
                "c_user={}; xs={}; datr={}; fr={};",
                session.c_user,
                session.xs,
                session.datr.as_deref().unwrap_or(""),
                session.fr.as_deref().unwrap_or("")
            );
            if let Ok(cookie_val) = HeaderValue::from_str(&cookie_str) {
                headers.insert(COOKIE, cookie_val);
            }
        }

        let resp = self.client.get(post_url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| format!("Lỗi tải trang Facebook: {}", e))?;

        let html = resp.text().await
            .map_err(|e| format!("Lỗi đọc nội dung HTML: {}", e))?;

        Ok(Self::parse_post_details_from_html(&html, post_id))
    }

    /// Parses author name, description/message, feedbackTargetID, and embedded comments from HTML
    pub fn parse_post_details_from_html(html: &str, post_id: &str) -> PostDetails {
        // 1. Author Name
        let mut author_name: Option<String> = None;
        if let Ok(re) = Regex::new(r#"<meta\s+property="og:title"\s+content="([^"]*)""#) {
            if let Some(cap) = re.captures(html) {
                if let Some(m) = cap.get(1) {
                    let raw = decode_entities(m.as_str().trim());
                    if !raw.is_empty() && raw != "Facebook" && !raw.starts_with("Log into") && !raw.starts_with("Đăng nhập") {
                        let cleaned = raw
                            .replace(" - Posts | Facebook", "")
                            .replace(" - Bài viết | Facebook", "")
                            .replace(" | Facebook", "")
                            .replace(" - Trang chủ | Facebook", "");
                        author_name = Some(cleaned.trim().to_string());
                    }
                }
            }
        }
        if author_name.is_none() {
            if let Ok(re) = Regex::new(r#""profile_name":"([^"]+)""#) {
                if let Some(cap) = re.captures(html) {
                    if let Some(m) = cap.get(1) {
                        let raw = decode_entities(m.as_str().trim());
                        if !raw.is_empty() && raw != "Facebook" {
                            author_name = Some(raw);
                        }
                    }
                }
            }
        }
        if author_name.is_none() {
            if let Ok(re) = Regex::new(r#"<title>([^<]+)</title>"#) {
                if let Some(cap) = re.captures(html) {
                    if let Some(m) = cap.get(1) {
                        let raw = decode_entities(m.as_str().trim());
                        if !raw.is_empty() && raw != "Facebook" && !raw.starts_with("Log into") && !raw.starts_with("Đăng nhập") {
                            let cleaned = raw
                                .replace(" - Posts | Facebook", "")
                                .replace(" - Bài viết | Facebook", "")
                                .replace(" | Facebook", "");
                            author_name = Some(cleaned.trim().to_string());
                        }
                    }
                }
            }
        }

        // 2. Content Preview
        let mut content_preview: Option<String> = None;
        if let Ok(re) = Regex::new(r#"<meta\s+property="og:description"\s+content="([^"]*)""#) {
            if let Some(cap) = re.captures(html) {
                if let Some(m) = cap.get(1) {
                    let raw = decode_entities(m.as_str().trim());
                    if !raw.is_empty() && raw != "Facebook" {
                        content_preview = Some(raw);
                    }
                }
            }
        }
        if content_preview.is_none() {
            if let Ok(re) = Regex::new(r#""meta":\{"title":"([^"]+)""#) {
                if let Some(cap) = re.captures(html) {
                    if let Some(m) = cap.get(1) {
                        let raw = decode_entities(m.as_str().trim());
                        if !raw.is_empty() {
                            content_preview = Some(raw);
                        }
                    }
                }
            }
        }
        if content_preview.is_none() {
            if let Ok(re) = Regex::new(r#""message":\{"text":"((?:[^"\\]|\\.)*)""#) {
                if let Some(cap) = re.captures(html) {
                    if let Some(m) = cap.get(1) {
                        let raw = decode_entities(m.as_str().trim());
                        if !raw.is_empty() {
                            content_preview = Some(raw);
                        }
                    }
                }
            }
        }

        // Format preview length if too long
        if let Some(prev) = &content_preview {
            if prev.chars().count() > 200 {
                let truncated: String = prev.chars().take(200).collect();
                content_preview = Some(format!("{}...", truncated));
            }
        }

        // 3. Resolved Feedback / Post Target ID
        let mut resolved_id: Option<String> = None;

        // Try extracting from storyID (e.g. UzpfSTEwMDA... -> S:_I1000...:1657711862379521:1657711862379521)
        if let Ok(re) = Regex::new(r#""storyID":"([^"]+)""#) {
            for cap in re.captures_iter(html) {
                if let Some(m) = cap.get(1) {
                    if let Ok(decoded_bytes) = BASE64_STANDARD.decode(m.as_str()) {
                        if let Ok(decoded_str) = String::from_utf8(decoded_bytes) {
                            let parts: Vec<&str> = decoded_str.split(':').collect();
                            if let Some(last_id) = parts.last() {
                                if last_id.chars().all(|c| c.is_ascii_digit()) && last_id.len() >= 8 {
                                    resolved_id = Some(last_id.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Try extracting from mediaset_token (pcb.<id>)
        if resolved_id.is_none() {
            if let Ok(re) = Regex::new(r#""mediaset_token":"pcb\.(\d+)""#) {
                if let Some(cap) = re.captures(html) {
                    if let Some(m) = cap.get(1) {
                        resolved_id = Some(m.as_str().to_string());
                    }
                }
            }
        }

        // Try other post-specific patterns (excluding comment-level legacy_fbid)
        if resolved_id.is_none() {
            let id_patterns = [
                r#""feedbackTargetID":"(\d+)""#,
                r#""story_token":"(\d+)""#,
                r#""target_id":"(\d+)""#,
                r#""story_fbid":\["(\d+)"\]"#,
                r#"(?:fbid|story_fbid)=(\d+)"#,
                r#""ft_ent_identifier":"(\d+)""#,
            ];
            for pat in id_patterns {
                if let Ok(re) = Regex::new(pat) {
                    if let Some(cap) = re.captures(html) {
                        if let Some(m) = cap.get(1) {
                            let id_str = m.as_str().to_string();
                            if !id_str.is_empty() {
                                resolved_id = Some(id_str);
                                break;
                            }
                        }
                    }
                }
            }
        }

        // 4. Extract fb_dtsg if embedded in the page
        let mut fb_dtsg: Option<String> = None;
        if let Ok(re) = Regex::new(r#""DTSGInitialData",\[\],\{"token":"([^"]+)""#) {
            if let Some(cap) = re.captures(html) {
                if let Some(m) = cap.get(1) {
                    fb_dtsg = Some(m.as_str().to_string());
                }
            }
        }
        if fb_dtsg.is_none() {
            if let Ok(re) = Regex::new(r#"name="fb_dtsg"\s+value="([^"]+)""#) {
                if let Some(cap) = re.captures(html) {
                    if let Some(m) = cap.get(1) {
                        fb_dtsg = Some(m.as_str().to_string());
                    }
                }
            }
        }

        // 5. Extract initial comments, post target ID, AND total comment count from embedded Relay JSON in <script> tags
        let mut initial_comments = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();
        let mut reply_targets = Vec::new();
        let mut seen_targets = std::collections::HashSet::new();
        let mut total_comment_count: Option<i64> = None;
        if let Ok(script_re) = Regex::new(r#"(?s)<script[^>]*type="application/json"[^>]*>(.*?)</script>"#) {
            for cap in script_re.captures_iter(html) {
                if let Some(script_content) = cap.get(1) {
                    let content_str = script_content.as_str().trim();
                    if content_str.contains("feedback") || content_str.contains("comments") || content_str.contains("Comment") || content_str.contains("body") {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(content_str) {
                            // Extract target ID if not resolved yet (essential for /share/ URLs)
                            if resolved_id.is_none() {
                                if let Some(target) = Self::find_post_feedback_id_recursive(&val) {
                                    resolved_id = Some(target);
                                }
                            }

                            Self::find_comment_nodes_and_targets(&val, post_id, &mut initial_comments, &mut seen_ids, &mut reply_targets, &mut seen_targets);
                            // Try to extract total comment count from this script block
                            if total_comment_count.is_none() {
                                total_comment_count = Self::find_total_comment_count_recursive(&val);
                            }
                        }
                    }
                }
            }
        }

        // Fallback: regex patterns for total_count in raw HTML
        if total_comment_count.is_none() {
            let count_patterns = [
                r#""total_comment_count":(\d+)"#,
                r#""comment_count":\{"total_count":(\d+)"#,
                r#""comments":\{"total_count":(\d+)"#,
                r#""feedback":\{[^}]*"comment_count":(\d+)"#,
            ];
            for pat in count_patterns {
                if let Ok(re) = Regex::new(pat) {
                    if let Some(cap) = re.captures(html) {
                        if let Some(m) = cap.get(1) {
                            if let Ok(n) = m.as_str().parse::<i64>() {
                                if n > 0 {
                                    total_comment_count = Some(n);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(count) = total_comment_count {
            eprintln!("[Crawler] Post {} has {} total comments reported by Facebook (target_id={:?})", post_id, count, resolved_id);
        }

        PostDetails {
            resolved_id,
            author_name,
            content_preview,
            initial_comments,
            fb_dtsg,
            total_comment_count,
            reply_targets,
        }
    }

    /// Recursively searches JSON tree for post-level feedback ID (useful for share links)
    fn find_post_feedback_id_recursive(val: &serde_json::Value) -> Option<String> {
        match val {
            serde_json::Value::Object(map) => {
                if let Some(fb) = map.get("feedback") {
                    if let Some(id_str) = fb.get("id").and_then(|v| v.as_str()) {
                        if let Ok(decoded_bytes) = BASE64_STANDARD.decode(id_str) {
                            if let Ok(decoded) = String::from_utf8(decoded_bytes) {
                                if let Some(target) = decoded.strip_prefix("feedback:") {
                                    if !target.contains('_') && target.chars().all(|c| c.is_ascii_digit()) && target.len() >= 8 {
                                        return Some(target.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(ft) = map.get("feedbackTargetID").and_then(|v| v.as_str()) {
                    if ft.chars().all(|c| c.is_ascii_digit()) && ft.len() >= 8 {
                        return Some(ft.to_string());
                    }
                }
                for v in map.values() {
                    if let Some(res) = Self::find_post_feedback_id_recursive(v) {
                        return Some(res);
                    }
                }
                None
            }
            serde_json::Value::Array(arr) => {
                for item in arr {
                    if let Some(res) = Self::find_post_feedback_id_recursive(item) {
                        return Some(res);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Recursively searches JSON for Facebook's comment total_count field
    /// Looks for patterns like: {"total_count": N} nested under "comments", "feedback", etc.
    fn find_total_comment_count_recursive(val: &serde_json::Value) -> Option<i64> {
        match val {
            serde_json::Value::Object(map) => {
                // Check if this object has total_count and it's under a comments/feedback context
                if let Some(tc) = map.get("total_count").and_then(|v| v.as_i64()) {
                    // Must be a reasonable comment count (>0 and not an ID-like giant number)
                    if tc > 0 && tc < 10_000_000 {
                        // Check parent context - should be near "comments" or "feedback" key
                        if map.contains_key("edges") || map.contains_key("page_info") ||
                           map.contains_key("comment_count") || map.contains_key("feedback") {
                            return Some(tc);
                        }
                    }
                }
                // Direct comment_count field
                if let Some(cc) = map.get("comment_count") {
                    if let Some(tc) = cc.as_i64() {
                        if tc > 0 { return Some(tc); }
                    }
                    if let Some(tc) = cc.get("total_count").and_then(|v| v.as_i64()) {
                        if tc > 0 { return Some(tc); }
                    }
                }
                for v in map.values() {
                    if let Some(n) = Self::find_total_comment_count_recursive(v) {
                        return Some(n);
                    }
                }
                None
            }
            serde_json::Value::Array(arr) => {
                for item in arr {
                    if let Some(n) = Self::find_total_comment_count_recursive(item) {
                        return Some(n);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Recursively traverses any JSON tree to extract Facebook comment items and their sub-replies
    pub fn find_comment_nodes(
        val: &serde_json::Value,
        post_id: &str,
        comments: &mut Vec<CommentItem>,
        seen_ids: &mut std::collections::HashSet<String>,
    ) {
        let mut dummy_targets = Vec::new();
        let mut dummy_seen_targets = std::collections::HashSet::new();
        Self::find_comment_nodes_recursive(val, post_id, None, comments, seen_ids, &mut dummy_targets, &mut dummy_seen_targets);
    }

    /// Recursively traverses JSON tree to extract comments AND reply targets (for sub-comment fetching)
    pub fn find_comment_nodes_and_targets(
        val: &serde_json::Value,
        post_id: &str,
        comments: &mut Vec<CommentItem>,
        seen_ids: &mut std::collections::HashSet<String>,
        reply_targets: &mut Vec<CommentReplyTarget>,
        seen_targets: &mut std::collections::HashSet<String>,
    ) {
        Self::find_comment_nodes_recursive(val, post_id, None, comments, seen_ids, reply_targets, seen_targets);
    }

    fn find_comment_nodes_recursive(
        val: &serde_json::Value,
        post_id: &str,
        parent_id_context: Option<&str>,
        comments: &mut Vec<CommentItem>,
        seen_ids: &mut std::collections::HashSet<String>,
        reply_targets: &mut Vec<CommentReplyTarget>,
        seen_targets: &mut std::collections::HashSet<String>,
    ) {
        match val {
            serde_json::Value::Object(map) => {
                let id_opt = map.get("legacy_fbid").and_then(|v| {
                    if let Some(s) = v.as_str() { Some(s.to_string()) }
                    else if let Some(n) = v.as_i64() { Some(n.to_string()) }
                    else { None }
                }).or_else(|| map.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()));

                let mut is_comment_node = false;
                let mut current_comment_id: Option<String> = None;

                if let Some(id_val) = id_opt {
                    let has_body = map.contains_key("body") || map.contains_key("message") || map.contains_key("body_renderer") || map.contains_key("attachments");
                    let has_author = map.contains_key("author");
                    // Real comments always have a created_time integer epoch.
                    let has_created_time = map.get("created_time").and_then(|v| v.as_i64()).is_some();
                    if has_body && has_author && has_created_time {
                        is_comment_node = true;
                        current_comment_id = Some(id_val.clone());
                        if !seen_ids.contains(&id_val) {
                            if let Some(item) = Self::extract_comment_item(val, post_id, parent_id_context) {
                                seen_ids.insert(item.id.clone());
                                comments.push(item);
                            }
                        }

                        // Collect reply target if this comment reports replies
                        if let Some(fb) = map.get("feedback") {
                            let fbid = fb.get("id").and_then(|v| v.as_str());
                            let exp_token = fb.get("expansion_info")
                                .or_else(|| map.get("expansion_info"))
                                .and_then(|e| e.get("expansion_token"))
                                .and_then(|v| v.as_str());
                            let count = fb.get("replies_fields")
                                .or_else(|| map.get("replies_fields"))
                                .and_then(|r| r.get("total_count").or_else(|| r.get("count")))
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0);

                            if count > 0 {
                                if let (Some(fid), Some(tok)) = (fbid, exp_token) {
                                    if !seen_targets.contains(&id_val) {
                                        seen_targets.insert(id_val.clone());
                                        reply_targets.push(CommentReplyTarget {
                                            parent_id: id_val.clone(),
                                            feedback_id: fid.to_string(),
                                            expansion_token: tok.to_string(),
                                            replies_count: count,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }

                let this_id_ref = current_comment_id.as_deref().or(parent_id_context);

                for (key, v) in map {
                    // When traversing into reply/sub-comment collections of this comment, propagate parent ID
                    let child_parent_ctx = if is_comment_node && (
                        key.contains("repl") ||
                        key.contains("threaded") ||
                        key == "feedback" ||
                        key == "comments"
                    ) {
                        this_id_ref
                    } else {
                        parent_id_context
                    };

                    Self::find_comment_nodes_recursive(v, post_id, child_parent_ctx, comments, seen_ids, reply_targets, seen_targets);
                }
            }
            serde_json::Value::Array(arr) => {
                for item in arr {
                    Self::find_comment_nodes_recursive(item, post_id, parent_id_context, comments, seen_ids, reply_targets, seen_targets);
                }
            }
            _ => {}
        }
    }

    /// Jitter delay: 0.8s - 1.8s to mimic human pacing while staying fast enough for bulk crawling
    pub async fn jitter_sleep() {
        let millis = {
            let mut rng = rand::thread_rng();
            rng.gen_range(800..=1800)
        };
        tokio::time::sleep(Duration::from_millis(millis)).await;
    }

    /// Extracts Vietnamese phone numbers from text
    pub fn extract_phone_numbers(text: &str) -> Option<String> {
        // Obfuscated regex string for Vietnamese telephone numbers
        // Formats: 09xx, 08xx, 07xx, 03xx, 05xx, +84, with dots, spaces, dashes
        let pattern = obfstring!(r"(?:(?:\+84|84|0)(?:3[2-9]|5[6|8|9]|7[0|6-9]|8[1-9]|9[0-9]))[\s.-]?\d{3}[\s.-]?\d{3,4}");
        if let Ok(re) = Regex::new(&pattern) {
            let mut phones = Vec::new();
            for mat in re.find_iter(text) {
                let clean = mat.as_str()
                    .replace([' ', '.', '-', '+'], "");
                // Standardize prefix to 0
                let normalized = if clean.starts_with("84") {
                    format!("0{}", &clean[2..])
                } else {
                    clean
                };
                if !phones.contains(&normalized) {
                    phones.push(normalized);
                }
            }
            if !phones.is_empty() {
                return Some(phones.join(", "));
            }
        }
        None
    }

    /// Classifies user comment intent into: '[Chốt đơn]', '[Hỏi giá]', '[Khiếu nại]', '[Spam]'
    pub fn classify_intent(text: &str, has_phone: bool) -> String {
        let lower = text.to_lowercase();

        // Spam indicators
        let spam_words = ["zalo.me", "t.me/", "telegram", "kiếm tiền", "cờ bạc", "tài xỉu", "lô đề", "ib riêng tele"];
        for w in spam_words {
            if lower.contains(w) {
                return "[Spam]".to_string();
            }
        }

        // Complaint indicators
        let complaint_words = ["lừa đảo", "chưa nhận được", "sai hàng", "hàng lỗi", "hoàn tiền", "kém chất lượng", "đổi hàng", "rách", "hỏng", "shop làm ăn"];
        for w in complaint_words {
            if lower.contains(w) {
                return "[Khiếu nại]".to_string();
            }
        }

        // Order closing indicators (Explicit or has phone number + order action)
        if has_phone {
            return "[Chốt đơn]".to_string();
        }
        let order_words = ["chốt", "đặt", "lấy", "ship", "giao", "size", "màu", "cho 1 cái", "cho 2 cái", "cho một", "mua", "địa chỉ"];
        for w in order_words {
            if lower.contains(w) {
                return "[Chốt đơn]".to_string();
            }
        }

        // Price inquiry indicators
        let price_words = ["giá", "nhiêu", "bao nhiêu", "bn", "inbox giá", "ib giá", "cost", "tư vấn", "shop ơi còn không", "còn hàng không"];
        for w in price_words {
            if lower.contains(w) {
                return "[Hỏi giá]".to_string();
            }
        }

        "[Hỏi giá]".to_string()
    }

    /// High performance GraphQL internal comment crawler using HTTP/2 and realistic headers
    pub async fn fetch_comments_page(
        &self,
        post_id: &str,
        cursor: Option<&str>,
    ) -> Result<CrawlPageResult, String> {
        let session = self.session.read().await.clone()
            .ok_or_else(|| "Chưa đăng nhập Facebook session trong ứng dụng.".to_string())?;

        let mut headers = HeaderMap::new();

        // Realistic Browser User-Agent and sec-ch-ua headers
        let user_agent = obfstring!("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36");
        if let Ok(val) = HeaderValue::from_str(&user_agent) {
            headers.insert(USER_AGENT, val);
        }
        headers.insert("sec-ch-ua", HeaderValue::from_static("\"Chromium\";v=\"124\", \"Google Chrome\";v=\"124\", \"Not-A.Brand\";v=\"99\""));
        headers.insert("sec-ch-ua-mobile", HeaderValue::from_static("?0"));
        headers.insert("sec-ch-ua-platform", HeaderValue::from_static("\"macOS\""));
        headers.insert("sec-fetch-dest", HeaderValue::from_static("empty"));
        headers.insert("sec-fetch-mode", HeaderValue::from_static("cors"));
        headers.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        headers.insert("origin", HeaderValue::from_static("https://www.facebook.com"));
        headers.insert("referer", HeaderValue::from_static("https://www.facebook.com/"));

        // Format session cookies
        let cookie_str = format!(
            "c_user={}; xs={}; datr={}; fr={};",
            session.c_user,
            session.xs,
            session.datr.as_deref().unwrap_or(""),
            session.fr.as_deref().unwrap_or("")
        );
        if let Ok(cookie_val) = HeaderValue::from_str(&cookie_str) {
            headers.insert(COOKIE, cookie_val);
        }

        // Facebook GraphQL internal endpoint
        let endpoint = obfstring!("https://www.facebook.com/api/graphql/");

        // Convert post_id / target_id to feedback ID in Base64 if needed
        let feedback_id = if post_id.starts_with("feedback:") {
            BASE64_STANDARD.encode(post_id)
        } else if post_id.len() > 25 && !post_id.chars().all(|c| c.is_ascii_digit()) {
            post_id.to_string()
        } else {
            BASE64_STANDARD.encode(format!("feedback:{}", post_id))
        };

        // Build variables for CommentsListComponentsPaginationQuery
        let variables = serde_json::json!({
            "commentsAfterCount": 50,
            "commentsAfterCursor": cursor,
            "commentsBeforeCount": serde_json::Value::Null,
            "commentsBeforeCursor": serde_json::Value::Null,
            "commentsIntentToken": "RANKED_UNFILTERED_CHRONOLOGICAL_REPLIES_INTENT_V1",
            "feedLocation": "POST_PERMALINK_DIALOG",
            "focusCommentID": serde_json::Value::Null,
            "id": feedback_id,
            "scale": 1,
            "targetDialect": serde_json::Value::Null,
            "useDefaultActor": false,
            "__relay_internal__pv__CometUFICommentAutoTranslationTyperelayprovider": "AUTO_TRANSLATE",
            "__relay_internal__pv__CometUFICommentAvatarStickerAnimatedImagerelayprovider": false,
            "__relay_internal__pv__CometUFICommentActionLinksRewriteEnabledrelayprovider": false,
            "__relay_internal__pv__IsWorkUserrelayprovider": false
        });

        let mut params = std::collections::HashMap::new();
        // Active doc_id for modern CommentsListComponentsPaginationQuery
        let doc_id = obfstring!("38580553541588665");
        params.insert("doc_id", doc_id.clone());
        params.insert("fb_dtsg", session.fb_dtsg.clone());
        let vars_str = variables.to_string();
        params.insert("variables", vars_str);

        let resp = self.client.post(&endpoint)
            .headers(headers.clone())
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Lỗi gửi request GraphQL: {}", e))?;

        let status = resp.status();
        if status.as_u16() == 429 {
            return Ok(CrawlPageResult {
                comments: vec![],
                next_cursor: None,
                has_next_page: false,
                is_checkpoint: false,
                is_rate_limited: true,
                total_count: None,
                reply_targets: vec![],
            });
        }

        let mut resp_text = resp.text().await.map_err(|e| format!("Lỗi đọc body GraphQL: {}", e))?;

        // Fallback retry with null intent token if modern token encounters a field_exception
        if resp_text.contains("field_exception") {
            let mut fallback_vars = variables.clone();
            fallback_vars["commentsIntentToken"] = serde_json::Value::Null;
            params.insert("variables", fallback_vars.to_string());
            if let Ok(retry_resp) = self.client.post(&endpoint).headers(headers).form(&params).send().await {
                if let Ok(retry_text) = retry_resp.text().await {
                    if !retry_text.contains("field_exception") {
                        resp_text = retry_text;
                    }
                }
            }
        }

        // Genuine checkpoint or login redirection detection
        let is_checkpoint = resp_text.contains("/checkpoint/start")
            || resp_text.contains("\"login_error\"")
            || resp_text.contains("action_redirect\":\"/checkpoint/")
            || (resp_text.starts_with("<!DOCTYPE html") && resp_text.contains("login.php"));

        if is_checkpoint {
            return Ok(CrawlPageResult {
                comments: vec![],
                next_cursor: None,
                has_next_page: false,
                is_checkpoint: true,
                is_rate_limited: false,
                total_count: None,
                reply_targets: vec![],
            });
        }

        // Parse comments from response JSON
        let parsed_comments = Self::parse_graphql_response(&resp_text, post_id)?;
        Ok(parsed_comments)
    }

    /// Fetches all sub-comments (replies) for a specific parent comment using Depth1CommentsListPaginationQuery
    pub async fn fetch_comment_replies(
        &self,
        post_id: &str,
        parent_id: &str,
        feedback_id: &str,
        expansion_token: &str,
    ) -> Result<Vec<CommentItem>, String> {
        let session = self.session.read().await.clone()
            .ok_or_else(|| "Chưa đăng nhập Facebook session trong ứng dụng.".to_string())?;

        let mut headers = HeaderMap::new();
        let user_agent = obfstring!("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36");
        if let Ok(val) = HeaderValue::from_str(&user_agent) {
            headers.insert(USER_AGENT, val);
        }
        headers.insert("sec-ch-ua", HeaderValue::from_static("\"Chromium\";v=\"124\", \"Google Chrome\";v=\"124\", \"Not-A.Brand\";v=\"99\""));
        headers.insert("sec-ch-ua-mobile", HeaderValue::from_static("?0"));
        headers.insert("sec-ch-ua-platform", HeaderValue::from_static("\"macOS\""));
        headers.insert("sec-fetch-dest", HeaderValue::from_static("empty"));
        headers.insert("sec-fetch-mode", HeaderValue::from_static("cors"));
        headers.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        headers.insert("origin", HeaderValue::from_static("https://www.facebook.com"));
        headers.insert("referer", HeaderValue::from_static("https://www.facebook.com/"));

        let cookie_str = format!(
            "c_user={}; xs={}; datr={}; fr={};",
            session.c_user,
            session.xs,
            session.datr.as_deref().unwrap_or(""),
            session.fr.as_deref().unwrap_or("")
        );
        if let Ok(cookie_val) = HeaderValue::from_str(&cookie_str) {
            headers.insert(COOKIE, cookie_val);
        }

        let endpoint = obfstring!("https://www.facebook.com/api/graphql/");
        // Depth1CommentsListPaginationQuery doc_id from Comet bundle
        let doc_id = obfstring!("39246134745000506");

        let mut all_replies = Vec::new();
        let mut current_cursor: Option<String> = None;
        let mut page = 0;
        let max_pages = 10; // Safety cap: up to 500 replies per parent comment

        loop {
            if page > 0 {
                Self::jitter_sleep().await;
            }
            page += 1;

            let variables = serde_json::json!({
                "clientKey": serde_json::Value::Null,
                "expansionToken": expansion_token,
                "feedLocation": "POST_PERMALINK_DIALOG",
                "focusCommentID": serde_json::Value::Null,
                "id": feedback_id,
                "repliesAfterCount": 50,
                "repliesAfterCursor": current_cursor,
                "repliesBeforeCount": serde_json::Value::Null,
                "repliesBeforeCursor": serde_json::Value::Null,
                "scale": 1,
                "useDefaultActor": false,
                "__relay_internal__pv__CometUFICommentAutoTranslationTyperelayprovider": "AUTO_TRANSLATE",
                "__relay_internal__pv__CometUFICommentAvatarStickerAnimatedImagerelayprovider": false,
                "__relay_internal__pv__CometUFICommentActionLinksRewriteEnabledrelayprovider": false,
                "__relay_internal__pv__IsWorkUserrelayprovider": false
            });

            let mut params = std::collections::HashMap::new();
            params.insert("doc_id", doc_id.clone());
            params.insert("fb_dtsg", session.fb_dtsg.clone());
            params.insert("variables", variables.to_string());

            let resp = self.client.post(&endpoint)
                .headers(headers.clone())
                .form(&params)
                .send()
                .await
                .map_err(|e| format!("Lỗi request GraphQL replies: {}", e))?;

            if resp.status().as_u16() == 429 {
                eprintln!("[Crawler Warning] 429 Rate limited khi lấy replies cho parent {}", parent_id);
                break;
            }

            let resp_text = resp.text().await.map_err(|e| format!("Lỗi đọc body replies: {}", e))?;

            if resp_text.contains("/checkpoint/start") || resp_text.contains("\"login_error\"") {
                break;
            }

            let mut has_next = false;
            let mut next_cur: Option<String> = None;
            let mut found_count = 0;

            for line in resp_text.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    if let Some(conn) = val.get("data")
                        .and_then(|d| d.get("node"))
                        .and_then(|n| n.get("replies_connection"))
                    {
                        if let Some(edges) = conn.get("edges").and_then(|e| e.as_array()) {
                            for edge in edges {
                                if let Some(node) = edge.get("node") {
                                    if let Some(item) = Self::extract_comment_item(node, post_id, Some(parent_id)) {
                                        all_replies.push(item);
                                        found_count += 1;
                                    }
                                }
                            }
                        }

                        if let Some(pi) = conn.get("page_info") {
                            has_next = pi.get("has_next_page").and_then(|v| v.as_bool()).unwrap_or(false);
                            next_cur = pi.get("end_cursor").and_then(|v| v.as_str()).map(|s| s.to_string());
                        }
                    }
                }
            }

            if has_next && next_cur.is_some() && page < max_pages && found_count > 0 {
                current_cursor = next_cur;
            } else {
                break;
            }
        }

        Ok(all_replies)
    }

    /// Parses Facebook GraphQL feedback comment JSON nodes across single-line or multi-line NDJSON
    pub fn parse_graphql_response(json_text: &str, post_id: &str) -> Result<CrawlPageResult, String> {
        let mut comments = Vec::new();
        let mut next_cursor: Option<String> = None;
        let mut has_next_page = false;
        let mut total_count: Option<i64> = None;
        let mut seen_ids = std::collections::HashSet::new();
        let mut reply_targets = Vec::new();
        let mut seen_targets = std::collections::HashSet::new();

        for line in json_text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                // Extract comments using recursive tree traversal
                Self::find_comment_nodes_and_targets(&val, post_id, &mut comments, &mut seen_ids, &mut reply_targets, &mut seen_targets);

                // Extract page_info recursively
                if next_cursor.is_none() {
                    if let Some((cur, has_next)) = Self::find_page_info_recursive(&val) {
                        next_cursor = Some(cur);
                        has_next_page = has_next;
                    }
                }

                // Extract total comment count from GraphQL response
                if total_count.is_none() {
                    total_count = Self::find_total_comment_count_recursive(&val);
                }
            }
        }

        if next_cursor.is_some() && !has_next_page {
            has_next_page = true;
        }

        Ok(CrawlPageResult {
            comments,
            next_cursor,
            has_next_page,
            is_checkpoint: false,
            is_rate_limited: false,
            total_count,
            reply_targets,
        })
    }

    fn find_page_info_recursive(val: &serde_json::Value) -> Option<(String, bool)> {
        match val {
            serde_json::Value::Object(map) => {
                if let Some(pi) = map.get("page_info") {
                    if let Some(end_cur) = pi.get("end_cursor").and_then(|v| v.as_str()) {
                        if !end_cur.is_empty() {
                            let has_next = pi.get("has_next_page").and_then(|v| v.as_bool()).unwrap_or(true);
                            return Some((end_cur.to_string(), has_next));
                        }
                    }
                }
                for v in map.values() {
                    if let Some(res) = Self::find_page_info_recursive(v) {
                        return Some(res);
                    }
                }
                None
            }
            serde_json::Value::Array(arr) => {
                for item in arr {
                    if let Some(res) = Self::find_page_info_recursive(item) {
                        return Some(res);
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn extract_comment_item(
        node: &serde_json::Value,
        post_id: &str,
        parent_id_context: Option<&str>,
    ) -> Option<CommentItem> {
        let id = if let Some(lid) = node.get("legacy_fbid") {
            if let Some(s) = lid.as_str() {
                s.to_string()
            } else if let Some(n) = lid.as_i64() {
                n.to_string()
            } else {
                node.get("id").and_then(|v| v.as_str())?.to_string()
            }
        } else {
            node.get("id").and_then(|v| v.as_str())?.to_string()
        };

        // Extract direct parent comment: prioritize parent_id_context (clean numeric ID),
        // or decode base64 / extract legacy_fbid from comment_direct_parent
        let parent_comment_id = parent_id_context.map(|s| s.to_string())
            .or_else(|| {
                node.get("comment_direct_parent").and_then(|p| {
                    if let Some(lid) = p.get("legacy_fbid") {
                        if let Some(s) = lid.as_str() { return Some(s.to_string()); }
                        if let Some(n) = lid.as_i64() { return Some(n.to_string()); }
                    }
                    if let Some(raw_id) = p.get("id").and_then(|v| v.as_str()) {
                        if let Ok(decoded_bytes) = BASE64_STANDARD.decode(raw_id) {
                            if let Ok(decoded_str) = String::from_utf8(decoded_bytes) {
                                if let Some(clean) = decoded_str.split('_').last() {
                                    if clean.chars().all(|c| c.is_ascii_digit()) && !clean.is_empty() {
                                        return Some(clean.to_string());
                                    }
                                }
                            }
                        }
                        Some(raw_id.to_string())
                    } else {
                        None
                    }
                })
            });

        let raw_content = node.get("body_renderer")
            .and_then(|b| b.get("text"))
            .and_then(|t| t.as_str())
            .or_else(|| node.get("body").and_then(|b| b.get("text")).and_then(|t| t.as_str()))
            .or_else(|| node.get("message").and_then(|m| m.get("text")).and_then(|t| t.as_str()))
            .unwrap_or("")
            .trim();

        let content = if raw_content.is_empty() {
            if node.get("attachments").is_some() || node.get("sticker").is_some() {
                "[Hình ảnh/Nhãn dán]".to_string()
            } else {
                "[Bình luận biểu cảm/nhãn dán]".to_string()
            }
        } else {
            decode_entities(raw_content)
        };

        let author = node.get("author");
        let author_id = author.and_then(|a| a.get("id")).and_then(|v| {
            if let Some(s) = v.as_str() {
                Some(s.to_string())
            } else if let Some(n) = v.as_i64() {
                Some(n.to_string())
            } else {
                None
            }
        });
        let author_name = author.and_then(|a| a.get("name")).and_then(|v| v.as_str()).map(|s| decode_entities(s.trim()));
        let author_url = author.and_then(|a| a.get("url")).and_then(|v| v.as_str()).map(|s| s.to_string())
            .or_else(|| author_id.as_ref().map(|aid| format!("https://www.facebook.com/{}", aid)));

        let created_at = node.get("created_time")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| Utc::now().timestamp());

        let phone_numbers = Self::extract_phone_numbers(&content);
        let has_phone = phone_numbers.is_some();
        let intent_tag = Self::classify_intent(&content, has_phone);

        Some(CommentItem {
            id,
            post_id: post_id.to_string(),
            parent_comment_id,
            author_id,
            author_name,
            author_url,
            content: Some(content),
            phone_numbers,
            intent_tag: Some(intent_tag),
            created_at: Some(created_at),
        })
    }
}
