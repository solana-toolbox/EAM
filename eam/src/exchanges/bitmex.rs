use crate::exchanges::monitor::ExchangeMonitor;
use crate::models::announcement::Announcement;
use anyhow::{Result, Context};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use reqwest::Client;

/// BitMEX announcement monitor
pub struct BitmexMonitor {
    client: Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct BitmexAnnouncementResponse {
    data: Vec<BitmexAnnouncement>,
    success: bool,
}

#[derive(Debug, Deserialize)]
struct BitmexAnnouncement {
    id: String,
    link: String,
    title: String,
    date: String,
    content: String,
}

impl BitmexMonitor {
    /// Create a new BitMEX monitor
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://www.bitmex.com/api/v1/announcement".to_string(),
        }
    }
}

#[async_trait]
impl ExchangeMonitor for BitmexMonitor {
    fn exchange_name(&self) -> &str {
        "BitMEX"
    }
    
    async fn fetch_announcements(&self) -> Result<Vec<Announcement>> {
        // Make the API request
        let response = self.client.get(&self.base_url)
            .send()
            .await
            .context("Failed to request BitMEX announcements")?;
        
        // Parse the response
        let bitmex_response: Vec<BitmexAnnouncement> = response.json()
            .await
            .context("Failed to parse BitMEX announcement response")?;
        
        // Convert BitMEX announcements to our standard format
        let mut announcements = Vec::new();
        for bitmex_announcement in bitmex_response {
            // Parse publish time - BitMEX uses ISO 8601 format
            let published_at = DateTime::parse_from_rfc3339(&bitmex_announcement.date)
                .unwrap_or_else(|_| Utc::now().into())
                .with_timezone(&Utc);
            
            // Create the announcement
            let url = if bitmex_announcement.link.starts_with("http") {
                bitmex_announcement.link
            } else {
                format!("https://www.bitmex.com{}", bitmex_announcement.link)
            };
            
            let mut announcement = Announcement::new(
                bitmex_announcement.id,
                bitmex_announcement.title,
                bitmex_announcement.content,
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
