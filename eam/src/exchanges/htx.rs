use crate::exchanges::monitor::ExchangeMonitor;
use crate::models::announcement::Announcement;
use crate::utils::{create_browser_client, retry_request, extract_response_data};
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc, TimeZone, NaiveDateTime};
use serde::Deserialize;
use reqwest::header;
use regex::Regex;

/// HTX announcement monitor (formerly Huobi)
pub struct HtxMonitor {
    client: reqwest::Client,
    api_url: String,
}

#[derive(Debug, Deserialize)]
struct HtxResponse {
    success: bool,
    code: i32,
    message: Option<String>,
    data: HtxData,
}

#[derive(Debug, Deserialize)]
struct HtxData {
    total: i32,
    list: Vec<HtxItem>,
}

#[derive(Debug, Deserialize)]
struct HtxItem {
    id: Option<String>,
    title: String,
    content: String,
    created_at: i64,
    lang: String,
}

impl HtxMonitor {
    /// Create a new HTX monitor
    pub fn new() -> Self {
        Self {
            client: match create_browser_client() {
                Ok(client) => client,
                Err(_) => reqwest::Client::new(),
            },
            api_url: "https://www.htx.com/api/v1/notice/get_notice_list".to_string(),
        }
    }
    
    /// Fetch the content for a specific announcement
    async fn fetch_announcement_content(&self, id: i64) -> Result<String> {
        let url = format!("https://www.htx.com/api/v1/notice/get_notice_by_id?id={}", id);
        let url_clone = url.clone();
        
        let headers = header::HeaderMap::new();
        headers.insert(header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36".parse().unwrap());
        
        // Use retry mechanism with proxy rotation
        let response = retry_request(
            move || {
                let client = create_browser_client().unwrap();
                let url = url_clone.clone();
                let headers = headers.clone();
                
                async move {
                    client.get(&url)
                        .headers(headers)
                        .send()
                        .await
                        .context("Failed to request HTX announcement content")
                }
            },
            3, // max retries
            500, // initial delay in ms
        ).await.context("Failed to fetch HTX announcement content after retries")?;
        
        // Use our new extract_response_data function with HTML fallback
        let content_response = extract_response_data::<HtxContentResponse>(
            response, 
            Some(|html| extract_htx_html_content(html))
        )
        .await
        .context("Failed to parse HTX announcement content")?;
        
        if !content_response.success {
            return Err(anyhow::anyhow!("HTX API returned error: {:?}", content_response.message));
        }
        
        Ok(content_response.data.content)
    }
    
    /// Parse date string to DateTime
    fn parse_date(&self, date_str: &str) -> DateTime<Utc> {
        // Try standard format first
        if let Ok(dt) = DateTime::parse_from_str(&format!("{} +0000", date_str), "%Y-%m-%d %H:%M:%S %z") {
            return dt.into();
        }
        
        // Fallback to just date
        if let Ok(date) = NaiveDateTime::parse_from_str(&format!("{} 00:00:00", date_str), "%Y-%m-%d %H:%M:%S") {
            return Utc.from_utc_datetime(&date);
        }
        
        // Return current time if parsing fails
        Utc::now()
    }
    
    async fn fetch_announcements(&self) -> Result<Vec<Announcement>> {
        let client = create_browser_client()?;
        
        let response = retry_request(
            || async {
                client
                    .get(&self.api_url)
                    .header(
                        header::USER_AGENT,
                        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36",
                    )
                    .send()
                    .await
                    .context("Failed to request HTX announcements")
            },
            3,
            1000,
        )
        .await
        .context("Failed to fetch HTX announcements after retries")?;
        
        // Use our new extract_response_data function with HTML fallback
        let htx_response = extract_response_data::<HtxResponse>(
            response, 
            Some(|html| extract_htx_html(html))
        )
        .await
        .context("Failed to parse HTX announcement response")?;
        
        if !htx_response.success {
            return Err(anyhow::anyhow!("HTX API returned error: {:?}", htx_response.message));
        }
        
        // Convert HTX announcements to our standard format
        let announcements = htx_response.data.list.into_iter()
            .map(|item| {
                // Convert timestamp to DateTime<Utc>
                let datetime = if item.created_at > 9999999999 {
                    // If the timestamp is in milliseconds
                    Utc.timestamp_millis_opt(item.created_at).single()
                        .unwrap_or_else(|| Utc::now())
                } else {
                    // If the timestamp is in seconds
                    Utc.timestamp_opt(item.created_at, 0).single()
                        .unwrap_or_else(|| Utc::now())
                };
                
                // Generate a UUID-like ID if none exists
                let id = item.id.unwrap_or_else(|| format!("htx-{}", chrono::Utc::now().timestamp()));
                
                Announcement {
                    id,
                    title: item.title,
                    content: item.content,
                    url: format!("https://www.htx.com/support/en-us/detail/{}", item.id.unwrap_or_default()),
                    exchange: "HTX".to_string(),
                    published_at: datetime,
                    is_new_listing: false, // Will be analyzed later
                    token_symbols: Vec::new(),
                }
            })
            .collect();
        
        Ok(announcements)
    }
}

/// Extract HTX announcements from HTML when API returns HTML instead of JSON
fn extract_htx_html(html: &str) -> Result<HtxResponse> {
    tracing::info!("Attempting to extract HTX announcements from HTML");
    
    // Simple regex to find announcement data in the HTML
    let re_pattern = r#"<div\s+class="article-item[^>]*>(.*?)</div>\s*</div>"#;
    let re = Regex::new(re_pattern).context("Failed to compile HTX HTML regex")?;
    
    // Title extractor regex
    let title_re = Regex::new(r#"<div[^>]*class="article-title[^>]*>(.*?)</div>"#)
        .context("Failed to compile HTX title regex")?;
    
    // Date extractor regex  
    let date_re = Regex::new(r#"<div[^>]*class="article-date[^>]*>(.*?)</div>"#)
        .context("Failed to compile HTX date regex")?;
    
    // Collect announcements from HTML
    let mut announcements = Vec::new();
    
    for cap in re.captures_iter(html) {
        if let Some(article_html) = cap.get(1) {
            let article_text = article_html.as_str();
            
            // Extract title
            let title = title_re.captures(article_text)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_else(|| "Unknown Title".to_string());
            
            // Extract date
            let date_str = date_re.captures(article_text)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim())
                .unwrap_or("2025-01-01");
            
            // Try to parse the date
            let created_at = NaiveDateTime::parse_from_str(date_str, "%Y-%m-%d")
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap().timestamp())
                .unwrap_or_else(|_| chrono::Utc::now().timestamp());
            
            announcements.push(HtxItem {
                id: None,
                title,
                content: "".to_string(),
                created_at,
                lang: "en_US".to_string(),
            });
        }
    }
    
    // If extraction failed or found no announcements
    if announcements.is_empty() {
        tracing::warn!("No announcements found in HTX HTML content");
        // Return empty successful response
        return Ok(HtxResponse {
            success: true,
            code: 200,
            message: None,
            data: HtxData {
                total: 0,
                list: Vec::new(),
            },
        });
    }
    
    // Return found announcements
    Ok(HtxResponse {
        success: true,
        code: 200,
        message: None,
        data: HtxData {
            total: announcements.len() as i32,
            list: announcements,
        },
    })
}

/// Extract HTX announcement content from HTML when API returns HTML instead of JSON
fn extract_htx_html_content(html: &str) -> Result<HtxContentResponse> {
    tracing::info!("Attempting to extract HTX announcement content from HTML");
    
    // Simple regex to find content data in the HTML
    let re_pattern = r#"<div\s+class="article-content[^>]*>(.*?)</div>"#;
    let re = Regex::new(re_pattern).context("Failed to compile HTX HTML regex")?;
    
    // Collect content from HTML
    let mut content = "";
    
    for cap in re.captures_iter(html) {
        if let Some(article_html) = cap.get(1) {
            let article_text = article_html.as_str();
            
            // Extract content
            content = article_text.trim();
        }
    }
    
    // If extraction failed or found no content
    if content.is_empty() {
        tracing::warn!("No content found in HTX HTML content");
        // Return empty successful response
        return Ok(HtxContentResponse {
            success: true,
            code: 200,
            message: None,
            data: HtxContentData {
                content: "".to_string(),
            },
        });
    }
    
    // Return found content
    Ok(HtxContentResponse {
        success: true,
        code: 200,
        message: None,
        data: HtxContentData {
            content: content.to_string(),
        },
    })
}

#[derive(Debug, Deserialize)]
struct HtxContentResponse {
    success: bool,
    code: i32,
    message: Option<String>,
    data: HtxContentData,
}

#[derive(Debug, Deserialize)]
struct HtxContentData {
    content: String,
}

#[async_trait]
impl ExchangeMonitor for HtxMonitor {
    fn exchange_name(&self) -> &str {
        "HTX"
    }
    
    async fn fetch_announcements(&self) -> Result<Vec<Announcement>> {
        self.fetch_announcements().await
    }
}
