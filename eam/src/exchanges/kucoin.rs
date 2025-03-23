use crate::exchanges::monitor::ExchangeMonitor;
use crate::models::announcement::Announcement;
use crate::utils::{create_browser_client, retry_request, extract_response_data};
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{Utc, TimeZone};
use serde::Deserialize;
use reqwest::header;
use regex::Regex;

/// KuCoin announcement monitor
pub struct KucoinMonitor {
    base_url: String,
    api_url: String,
}

#[derive(Debug, Deserialize)]
struct KucoinAnnouncementResponse {
    code: String,
    data: KucoinAnnouncementData,
}

#[derive(Debug, Deserialize)]
struct KucoinAnnouncementData {
    items: Vec<KucoinAnnouncement>,
    #[serde(rename = "totalPage")]
    total_page: i32,
    #[serde(rename = "pageSize")]
    page_size: i32,
    #[serde(rename = "currentPage")]
    current_page: i32,
    #[serde(rename = "totalNum")]
    total_num: i32,
}

#[derive(Debug, Deserialize)]
struct KucoinAnnouncement {
    id: String,
    title: String,
    summary: Option<String>,
    #[serde(rename = "publishedStartAt")]
    published_at: i64,
    #[serde(rename = "webPath")]
    web_path: String,
}

impl KucoinMonitor {
    /// Create a new KuCoin monitor
    pub fn new() -> Self {
        Self {
            base_url: "https://www.kucoin.com/api/v1/news/list".to_string(),
            api_url: "https://www.kucoin.com/_api/cms/articles?page=1&pageSize=20&category=listing&lang=en_US".to_string(),
        }
    }
    
    /// Fetch KuCoin announcements
    pub async fn fetch_announcements(&self) -> Result<Vec<Announcement>> {
        let client = match create_browser_client() {
            Ok(client) => client,
            Err(_) => reqwest::Client::new(),
        };
        
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
                    .context("Failed to request KuCoin announcements")
            },
            3,
            1000,
        )
        .await
        .context("Failed to fetch KuCoin announcements after retries")?;
        
        // Use our new extract_response_data function with HTML fallback
        let kucoin_response = extract_response_data::<KucoinAnnouncementResponse>(
            response, 
            Some(|html| extract_kucoin_html(html))
        )
        .await
        .context("Failed to parse KuCoin announcement response")?;
        
        if kucoin_response.code != "200000" {
            return Err(anyhow::anyhow!("KuCoin API returned error: {:?}", kucoin_response.code));
        }
        
        // Convert KuCoin announcements to our standard format
        let announcements = kucoin_response.data.items.into_iter()
            .map(|item| {
                // Convert Unix timestamp (in milliseconds) to DateTime<Utc>
                let datetime = if item.published_at > 9999999999 {
                    // If the timestamp is in milliseconds (more than 10 digits)
                    Utc.timestamp_millis_opt(item.published_at).single()
                        .unwrap_or_else(|| Utc::now())
                } else {
                    // If the timestamp is in seconds
                    Utc.timestamp_opt(item.published_at, 0).single()
                        .unwrap_or_else(|| Utc::now())
                };
                
                Announcement {
                    id: item.id,
                    title: item.title,
                    content: item.summary.unwrap_or_default(),
                    url: item.web_path,
                    exchange: "KuCoin".to_string(),
                    published_at: datetime,
                    is_new_listing: false, // Default, can be analyzed later
                    token_symbols: Vec::new(),
                }
            })
            .collect();
        
        Ok(announcements)
    }
}

/// Extract KuCoin announcements from HTML when API returns HTML instead of JSON
fn extract_kucoin_html(html: &str) -> Result<KucoinAnnouncementResponse> {
    tracing::info!("Attempting to extract KuCoin announcements from HTML");
    
    // Simple regex to find article data in script tags
    let re_pattern = r#"window\.__INITIAL_STATE__\s*=\s*(\{.*?\});"#;
    let re = Regex::new(re_pattern).context("Failed to compile KuCoin HTML regex")?;
    
    if let Some(captures) = re.captures(html) {
        if let Some(json_str) = captures.get(1) {
            let json_data = json_str.as_str();
            // Try to extract articles from the state object
            match serde_json::from_str::<serde_json::Value>(json_data) {
                Ok(state) => {
                    if let Some(articles) = state.get("news")
                        .and_then(|news| news.get("list"))
                        .and_then(|list| list.get("data")) 
                    {
                        // Try to convert the extracted articles to our format
                        let items = articles.as_array()
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|article| {
                                        let id = article.get("id")?.as_str()?.to_string();
                                        let title = article.get("title")?.as_str()?.to_string();
                                        let publish_date = article.get("publishDate")?.as_str()?.to_string();
                                        let url = format!("https://www.kucoin.com/news/{}", id);
                                        
                                        Some(KucoinAnnouncement {
                                            id: id.clone(),
                                            title,
                                            summary: None,
                                            published_at: publish_date.parse::<i64>().ok()?,
                                            web_path: url,
                                        })
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default();
                        
                        // Store the length for use in other fields
                        let items_len = items.len();
                        
                        return Ok(KucoinAnnouncementResponse {
                            code: "200000".to_string(),
                            data: KucoinAnnouncementData { 
                                items,
                                total_page: 1,
                                page_size: items_len as i32,
                                current_page: 1,
                                total_num: items_len as i32,
                            },
                        });
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to parse KuCoin HTML JSON data: {}", e);
                }
            }
        }
    }
    
    // If extraction failed, return empty response
    Ok(KucoinAnnouncementResponse {
        code: "200000".to_string(),
        data: KucoinAnnouncementData { 
            items: vec![],
            total_page: 1,
            page_size: 0,
            current_page: 1,
            total_num: 0,
        },
    })
}

#[async_trait]
impl ExchangeMonitor for KucoinMonitor {
    fn exchange_name(&self) -> &str {
        "KuCoin"
    }
    
    async fn fetch_announcements(&self) -> Result<Vec<Announcement>> {
        self.fetch_announcements().await
    }
}
