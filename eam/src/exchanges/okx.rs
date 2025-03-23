use crate::exchanges::monitor::ExchangeMonitor;
use crate::models::announcement::Announcement;
use anyhow::{Result, Context};
use async_trait::async_trait;
use chrono::{DateTime, Utc, TimeZone};
use serde::{Deserialize, Serialize};
use reqwest::Client;

/// OKX announcement monitor
pub struct OkxMonitor {
    client: Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct OkxAnnouncementResponse {
    code: String,
    msg: String,
    data: Vec<OkxAnnouncement>,
}

#[derive(Debug, Deserialize)]
struct OkxAnnouncement {
    #[serde(rename = "sTitle")]
    title: String,
    #[serde(rename = "iTime")]
    publish_time: String,
    #[serde(rename = "sWeburlpath")]
    url_path: String,
    #[serde(rename = "sContent")]
    content: Option<String>,
    #[serde(rename = "sCategoryName")]
    category: Option<String>,
}

impl OkxMonitor {
    /// Create a new OKX monitor
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://www.okx.com/v2/support/home/web/announcement/queryList".to_string(),
        }
    }

    /// Parse the OKX timestamp into a DateTime<Utc>
    fn parse_timestamp(&self, timestamp: &str) -> DateTime<Utc> {
        // OKX uses a format like "2023-06-09 10:11:16"
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S") {
            Utc.from_utc_datetime(&dt)
        } else {
            tracing::warn!(
                exchange = self.exchange_name(),
                timestamp = timestamp,
                "Failed to parse OKX timestamp"
            );
            Utc::now()
        }
    }
}

#[async_trait]
impl ExchangeMonitor for OkxMonitor {
    fn exchange_name(&self) -> &str {
        "OKX"
    }
    
    async fn fetch_announcements(&self) -> Result<Vec<Announcement>> {
        // Request parameters for OKX announcement API
        let params = [
            ("t", Utc::now().timestamp_millis().to_string()),
            ("language", "en_US".to_string()),
        ];
        
        // Make the API request
        let response = self.client.get(&self.base_url)
            .query(&params)
            .send()
            .await
            .context("Failed to request OKX announcements")?;
        
        // Parse the response
        let okx_response: OkxAnnouncementResponse = response.json()
            .await
            .context("Failed to parse OKX announcement response")?;
        
        // Check if the request was successful
        if okx_response.code != "0" {
            return Err(anyhow::anyhow!(
                "OKX API returned error: {}", okx_response.msg
            ));
        }
        
        // Convert OKX announcements to our standard format
        let mut announcements = Vec::new();
        for okx_announcement in okx_response.data {
            // Parse publish time
            let published_at = self.parse_timestamp(&okx_announcement.publish_time);
            
            // Construct the full URL
            let url = format!("https://www.okx.com{}", okx_announcement.url_path);
            
            // Generate a unique ID (OKX doesn't provide IDs directly)
            let id = format!("okx_{}", url.replace("/", "_"));
            
            // Get content, or use empty string if not available
            let content = okx_announcement.content.unwrap_or_default();
            
            // Create the announcement
            let mut announcement = Announcement::new(
                id,
                okx_announcement.title,
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
