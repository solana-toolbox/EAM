use crate::exchanges::monitor::ExchangeMonitor;
use crate::models::announcement::Announcement;
use anyhow::{Result, Context};
use async_trait::async_trait;
use chrono::{DateTime, Utc, TimeZone};
use serde::{Deserialize, Serialize};
use reqwest::Client;

/// MEXC announcement monitor
pub struct MexcMonitor {
    client: Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct MexcAnnouncementResponse {
    code: i32,
    data: MexcAnnouncementData,
    msg: String,
}

#[derive(Debug, Deserialize)]
struct MexcAnnouncementData {
    dataList: Vec<MexcAnnouncement>,
    total: i32,
}

#[derive(Debug, Deserialize)]
struct MexcAnnouncement {
    id: String,
    title: String,
    content: Option<String>,
    #[serde(rename = "createTime")]
    create_time: i64,
    url: Option<String>,
}

impl MexcMonitor {
    /// Create a new MEXC monitor
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://www.mexc.com/api/platform/notice/list".to_string(),
        }
    }
    
    /// Fetch the content for a specific announcement
    async fn fetch_announcement_content(&self, id: &str) -> Result<String> {
        let url = format!("https://www.mexc.com/api/platform/notice/detail?id={}", id);
        
        let response = self.client.get(&url)
            .send()
            .await
            .context("Failed to request MEXC announcement content")?;
        
        #[derive(Debug, Deserialize)]
        struct MexcContentResponse {
            code: i32,
            data: MexcContentData,
            msg: String,
        }
        
        #[derive(Debug, Deserialize)]
        struct MexcContentData {
            id: String,
            title: String,
            content: String,
        }
        
        let content_response: MexcContentResponse = response.json()
            .await
            .context("Failed to parse MEXC announcement content")?;
        
        if content_response.code != 200 {
            return Err(anyhow::anyhow!(
                "MEXC API returned error for content: {}", content_response.msg
            ));
        }
        
        Ok(content_response.data.content)
    }
}

#[async_trait]
impl ExchangeMonitor for MexcMonitor {
    fn exchange_name(&self) -> &str {
        "MEXC"
    }
    
    async fn fetch_announcements(&self) -> Result<Vec<Announcement>> {
        // Request parameters for the MEXC API
        let params = [
            ("pageNum", "1"),
            ("pageSize", "20"),
            ("catalogId", "5"), // New token listings category
            ("lang", "en_US"),
        ];
        
        // Make the API request
        let response = self.client.get(&self.base_url)
            .query(&params)
            .send()
            .await
            .context("Failed to request MEXC announcements")?;
        
        // Parse the response
        let mexc_response: MexcAnnouncementResponse = response.json()
            .await
            .context("Failed to parse MEXC announcement response")?;
        
        // Check if the request was successful
        if mexc_response.code != 200 {
            return Err(anyhow::anyhow!(
                "MEXC API returned error: {}", mexc_response.msg
            ));
        }
        
        // Convert MEXC announcements to our standard format
        let mut announcements = Vec::new();
        for mexc_announcement in mexc_response.data.dataList {
            // Convert timestamp to DateTime<Utc>
            let published_at = Utc.timestamp_opt(mexc_announcement.create_time / 1000, 0)
                .single()
                .unwrap_or_else(|| Utc::now());
            
            // Get content from the announcement or fetch it if not available
            let content = match mexc_announcement.content {
                Some(content) if !content.is_empty() => content,
                _ => match self.fetch_announcement_content(&mexc_announcement.id).await {
                    Ok(content) => content,
                    Err(e) => {
                        tracing::warn!(
                            exchange = self.exchange_name(),
                            announcement_id = &mexc_announcement.id,
                            error = %e,
                            "Failed to fetch MEXC announcement content"
                        );
                        String::new()
                    }
                }
            };
            
            // Get URL from the announcement or construct it
            let url = mexc_announcement.url.unwrap_or_else(|| 
                format!("https://www.mexc.com/support/notice/detail?id={}", mexc_announcement.id)
            );
            
            // Create the announcement
            let mut announcement = Announcement::new(
                mexc_announcement.id,
                mexc_announcement.title,
                content,
                url,
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
