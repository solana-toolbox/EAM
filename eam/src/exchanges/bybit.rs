use crate::exchanges::monitor::ExchangeMonitor;
use crate::models::announcement::Announcement;
use anyhow::{Result, Context};
use async_trait::async_trait;
use chrono::{DateTime, Utc, TimeZone};
use serde::{Deserialize, Serialize};
use reqwest::Client;

/// Bybit announcement monitor
pub struct BybitMonitor {
    client: Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct BybitAnnouncementResponse {
    success: bool,
    message: String,
    result: BybitAnnouncementResult,
}

#[derive(Debug, Deserialize)]
struct BybitAnnouncementResult {
    list: Vec<BybitAnnouncement>,
    total: usize,
}

#[derive(Debug, Deserialize)]
struct BybitAnnouncement {
    id: usize,
    title: String,
    #[serde(rename = "type")]
    announcement_type: String,
    #[serde(rename = "releaseDate")]
    release_date: String,
    description: String,
    url: String,
}

impl BybitMonitor {
    /// Create a new Bybit monitor
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://api2.bybit.com/announcement/api/v1/announcement/list".to_string(),
        }
    }
    
    /// Parse Bybit timestamp into DateTime<Utc>
    fn parse_timestamp(&self, timestamp: &str) -> DateTime<Utc> {
        // Bybit uses a format like "2023-06-09T10:11:16Z"
        if let Ok(dt) = DateTime::parse_from_rfc3339(timestamp) {
            dt.with_timezone(&Utc)
        } else {
            tracing::warn!(
                exchange = self.exchange_name(),
                timestamp = timestamp,
                "Failed to parse Bybit timestamp"
            );
            Utc::now()
        }
    }
}

#[async_trait]
impl ExchangeMonitor for BybitMonitor {
    fn exchange_name(&self) -> &str {
        "Bybit"
    }
    
    async fn fetch_announcements(&self) -> Result<Vec<Announcement>> {
        // Request parameters for the Bybit API
        let params = [
            ("locale", "en-US".to_string()),
            ("page", "1".to_string()),
            ("limit", "20".to_string()),
            ("type", "new_crypto".to_string()), // Filter for new crypto listings
        ];
        
        // Make the API request
        let response = self.client.get(&self.base_url)
            .query(&params)
            .send()
            .await
            .context("Failed to request Bybit announcements")?;
        
        // Parse the response
        let bybit_response: BybitAnnouncementResponse = response.json()
            .await
            .context("Failed to parse Bybit announcement response")?;
        
        // Check if the request was successful
        if !bybit_response.success {
            return Err(anyhow::anyhow!(
                "Bybit API returned error: {}", bybit_response.message
            ));
        }
        
        // Convert Bybit announcements to our standard format
        let mut announcements = Vec::new();
        for bybit_announcement in bybit_response.result.list {
            // Parse publish time
            let published_at = self.parse_timestamp(&bybit_announcement.release_date);
            
            // Create the announcement
            let mut announcement = Announcement::new(
                bybit_announcement.id.to_string(),
                bybit_announcement.title,
                bybit_announcement.description,
                bybit_announcement.url,
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
