use crate::exchanges::monitor::ExchangeMonitor;
use crate::models::announcement::Announcement;
use anyhow::{Result, Context};
use async_trait::async_trait;
use chrono::{DateTime, Utc, TimeZone};
use serde::{Deserialize, Serialize};
use reqwest::Client;

/// Bitget announcement monitor
pub struct BitgetMonitor {
    client: Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct BitgetAnnouncementResponse {
    code: String,
    msg: String,
    data: BitgetAnnouncementData,
}

#[derive(Debug, Deserialize)]
struct BitgetAnnouncementData {
    list: Vec<BitgetAnnouncement>,
    total: usize,
}

#[derive(Debug, Deserialize)]
struct BitgetAnnouncement {
    id: String,
    title: String,
    #[serde(rename = "releaseTime")]
    release_time: i64,
    url: String,
    content: Option<String>,
}

impl BitgetMonitor {
    /// Create a new Bitget monitor
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://api.bitget.com/api/v2/spot/public/support/notice/list".to_string(),
        }
    }

    /// Fetch full content for an announcement
    async fn fetch_announcement_content(&self, id: &str) -> Result<String> {
        let url = format!("https://api.bitget.com/api/v2/spot/public/support/notice/detail?id={}", id);
        
        let response = self.client.get(&url)
            .send()
            .await
            .context("Failed to request Bitget announcement detail")?;
        
        #[derive(Debug, Deserialize)]
        struct BitgetDetailResponse {
            code: String,
            msg: String,
            data: BitgetDetail,
        }
        
        #[derive(Debug, Deserialize)]
        struct BitgetDetail {
            id: String,
            title: String,
            content: String,
        }
        
        let detail_response: BitgetDetailResponse = response.json()
            .await
            .context("Failed to parse Bitget announcement detail")?;
        
        if detail_response.code != "00000" {
            return Err(anyhow::anyhow!(
                "Bitget API returned error for detail: {}", detail_response.msg
            ));
        }
        
        Ok(detail_response.data.content)
    }
}

#[async_trait]
impl ExchangeMonitor for BitgetMonitor {
    fn exchange_name(&self) -> &str {
        "Bitget"
    }
    
    async fn fetch_announcements(&self) -> Result<Vec<Announcement>> {
        // Request parameters for the Bitget API
        let params = [
            ("language", "en"),
            ("catalogId", "6"), // Listings category
            ("page", "1"),
            ("pageSize", "20"),
        ];
        
        // Make the API request
        let response = self.client.get(&self.base_url)
            .query(&params)
            .send()
            .await
            .context("Failed to request Bitget announcements")?;
        
        // Parse the response
        let bitget_response: BitgetAnnouncementResponse = response.json()
            .await
            .context("Failed to parse Bitget announcement response")?;
        
        // Check if the request was successful
        if bitget_response.code != "00000" {
            return Err(anyhow::anyhow!(
                "Bitget API returned error: {}", bitget_response.msg
            ));
        }
        
        // Convert Bitget announcements to our standard format
        let mut announcements = Vec::new();
        for bitget_announcement in bitget_response.data.list {
            // Convert timestamp to DateTime<Utc>
            let published_at = Utc.timestamp_opt(bitget_announcement.release_time / 1000, 0)
                .single()
                .unwrap_or_else(|| Utc::now());
            
            // Get content from the announcement or fetch it if not available
            let content = match bitget_announcement.content {
                Some(content) if !content.is_empty() => content,
                _ => match self.fetch_announcement_content(&bitget_announcement.id).await {
                    Ok(content) => content,
                    Err(e) => {
                        tracing::warn!(
                            exchange = self.exchange_name(),
                            announcement_id = bitget_announcement.id,
                            error = %e,
                            "Failed to fetch Bitget announcement content"
                        );
                        String::new()
                    }
                }
            };
            
            // Create the announcement
            let mut announcement = Announcement::new(
                bitget_announcement.id,
                bitget_announcement.title,
                content,
                bitget_announcement.url,
                self.exchange_name().to_string(),
                published_at,
            );
            
            // Analyze if this is a new listing
            announcement.analyze_for_new_listing();
            announcements.push(announcement);
        }
        
        Ok(announcements)
    }
}
