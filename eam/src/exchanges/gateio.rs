use crate::exchanges::monitor::ExchangeMonitor;
use crate::models::announcement::Announcement;
use anyhow::{Result, Context};
use async_trait::async_trait;
use chrono::{DateTime, Utc, TimeZone};
use serde::{Deserialize, Serialize};
use reqwest::Client;

/// Gate.io announcement monitor
pub struct GateioMonitor {
    client: Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct GateioAnnouncementResponse {
    code: i32,
    message: String,
    data: GateioData,
}

#[derive(Debug, Deserialize)]
struct GateioData {
    list: Vec<GateioAnnouncement>,
    total: usize,
}

#[derive(Debug, Deserialize)]
struct GateioAnnouncement {
    id: u64,
    title: String,
    content: String,
    #[serde(rename = "publishTime")]
    publish_time: u64,
    url: String,
}

impl GateioMonitor {
    /// Create a new Gate.io monitor
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://www.gate.io/api/v1/announcement/list".to_string(),
        }
    }
}

#[async_trait]
impl ExchangeMonitor for GateioMonitor {
    fn exchange_name(&self) -> &str {
        "Gate.io"
    }
    
    async fn fetch_announcements(&self) -> Result<Vec<Announcement>> {
        // Request parameters for the Gate.io API
        let params = [
            ("page", "1"),
            ("limit", "20"),
            ("lang", "en"),
            ("category", "listing"), // Focus on listing announcements
        ];
        
        // Make the API request
        let response = self.client.get(&self.base_url)
            .query(&params)
            .send()
            .await
            .context("Failed to request Gate.io announcements")?;
        
        // Parse the response
        let gateio_response: GateioAnnouncementResponse = response.json()
            .await
            .context("Failed to parse Gate.io announcement response")?;
        
        // Check if the request was successful
        if gateio_response.code != 0 {
            return Err(anyhow::anyhow!(
                "Gate.io API returned error: {}", gateio_response.message
            ));
        }
        
        // Convert Gate.io announcements to our standard format
        let mut announcements = Vec::new();
        for gateio_announcement in gateio_response.data.list {
            // Convert timestamp to DateTime<Utc>
            let published_at = Utc.timestamp_opt(gateio_announcement.publish_time as i64, 0)
                .single()
                .unwrap_or_else(|| Utc::now());
            
            // Create the announcement
            let mut announcement = Announcement::new(
                gateio_announcement.id.to_string(),
                gateio_announcement.title,
                gateio_announcement.content,
                gateio_announcement.url,
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
