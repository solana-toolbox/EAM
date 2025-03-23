use crate::exchanges::monitor::ExchangeMonitor;
use crate::models::announcement::Announcement;
use anyhow::{Result, Context};
use async_trait::async_trait;
use chrono::{DateTime, Utc, TimeZone};
use serde::{Deserialize, Serialize};
use reqwest::Client;

/// Upbit announcement monitor
pub struct UpbitMonitor {
    client: Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct UpbitAnnouncementResponse {
    success: bool,
    data: Vec<UpbitAnnouncement>,
}

#[derive(Debug, Deserialize)]
struct UpbitAnnouncement {
    id: u64,
    title: String,
    #[serde(rename = "created_at")]
    created_at: String,
    #[serde(rename = "view_count")]
    view_count: u64,
}

impl UpbitMonitor {
    /// Create a new Upbit monitor
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://api-manager.upbit.com/api/v1/notices".to_string(),
        }
    }
    
    /// Fetch the content for a specific announcement
    async fn fetch_announcement_content(&self, id: u64) -> Result<String> {
        let url = format!("https://api-manager.upbit.com/api/v1/notices/{}", id);
        
        let response = self.client.get(&url)
            .send()
            .await
            .context("Failed to request Upbit announcement details")?;
        
        #[derive(Debug, Deserialize)]
        struct UpbitAnnouncementDetail {
            success: bool,
            data: UpbitAnnouncementContent,
        }
        
        #[derive(Debug, Deserialize)]
        struct UpbitAnnouncementContent {
            id: u64,
            title: String,
            content: String,
            #[serde(rename = "created_at")]
            created_at: String,
        }
        
        let detail: UpbitAnnouncementDetail = response.json()
            .await
            .context("Failed to parse Upbit announcement detail")?;
        
        if !detail.success {
            return Err(anyhow::anyhow!("Upbit API returned unsuccessful response for announcement detail"));
        }
        
        Ok(detail.data.content)
    }
}

#[async_trait]
impl ExchangeMonitor for UpbitMonitor {
    fn exchange_name(&self) -> &str {
        "Upbit"
    }
    
    async fn fetch_announcements(&self) -> Result<Vec<Announcement>> {
        // Request parameters for the Upbit API
        let params = [
            ("page", "1"),
            ("per_page", "20"),
            ("thread_name", "general"), // General announcements
        ];
        
        // Make the API request
        let response = self.client.get(&self.base_url)
            .query(&params)
            .send()
            .await
            .context("Failed to request Upbit announcements")?;
        
        // Parse the response
        let upbit_response: UpbitAnnouncementResponse = response.json()
            .await
            .context("Failed to parse Upbit announcement response")?;
        
        // Check if the request was successful
        if !upbit_response.success {
            return Err(anyhow::anyhow!("Upbit API returned unsuccessful response"));
        }
        
        // Convert Upbit announcements to our standard format
        let mut announcements = Vec::new();
        for upbit_announcement in upbit_response.data {
            // Parse publish time - Upbit typically uses ISO 8601 format
            let published_at = DateTime::parse_from_rfc3339(&upbit_announcement.created_at)
                .unwrap_or_else(|_| Utc::now().into())
                .with_timezone(&Utc);
            
            // Construct the URL for the announcement
            let url = format!("https://upbit.com/service_center/notice?id={}", upbit_announcement.id);
            
            // Fetch the full content
            let content = match self.fetch_announcement_content(upbit_announcement.id).await {
                Ok(content) => content,
                Err(e) => {
                    tracing::warn!(
                        exchange = self.exchange_name(),
                        announcement_id = upbit_announcement.id,
                        error = %e,
                        "Failed to fetch Upbit announcement content"
                    );
                    String::new()
                }
            };
            
            // Create the announcement
            let mut announcement = Announcement::new(
                upbit_announcement.id.to_string(),
                upbit_announcement.title,
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
