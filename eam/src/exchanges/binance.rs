use crate::exchanges::monitor::ExchangeMonitor;
use crate::models::announcement::Announcement;
use crate::utils::{create_browser_headers, create_browser_client, retry_request, create_new_proxy_client};
use anyhow::{Result, Context};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use reqwest::Client;

/// Binance announcement monitor
pub struct BinanceMonitor {
    client: Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct BinanceAnnouncementResponse {
    code: String,
    message: Option<String>,
    data: Vec<BinanceAnnouncement>,
    total: usize,
    success: bool,
}

#[derive(Debug, Deserialize)]
struct BinanceAnnouncement {
    id: String,
    code: String,
    title: String,
    #[serde(rename = "type")]
    announcement_type: usize,
    #[serde(rename = "releaseDate")]
    release_date: i64,
    url: Option<String>,
}

impl BinanceMonitor {
    /// Create a new Binance monitor
    pub fn new() -> Self {
        Self {
            client: create_browser_client(),
            base_url: "https://www.binance.com/bapi/composite/v1/public/cms/article/catalog/list/query".to_string(),
        }
    }

    /// Fetch announcement content for a specific announcement ID
    async fn fetch_announcement_content(&self, url: &str) -> Result<String> {
        if let Some(url) = url.strip_prefix("https://www.binance.com") {
            let full_url = format!("https://www.binance.com{}", url);
            
            // Use retry mechanism for fetching content with proxy rotation
            let headers = create_browser_headers(None, Some("www.binance.com"));
            let full_url_clone = full_url.clone();
            
            let response = retry_request(
                move || {
                    // Create a new client with different proxy for each retry attempt
                    let client = create_new_proxy_client();
                    let url = full_url_clone.clone();
                    let headers = headers.clone();
                    async move {
                        client.get(&url)
                            .headers(headers)
                            .send()
                            .await
                            .context("Failed to request Binance announcement content")
                    }
                },
                3, // max retries 
                500, // initial delay in ms
            ).await.context("Failed to fetch Binance announcement content after retries")?;
            
            let html = response.text()
                .await
                .context("Failed to get Binance announcement HTML content")?;
            
            // Use scraper to extract the main content
            let document = scraper::Html::parse_document(&html);
            let content_selector = scraper::Selector::parse(".css-3iuet5").unwrap_or_else(|_| {
                // Fallback selector if the primary one changes
                scraper::Selector::parse("article").unwrap()
            });
            
            let content = document.select(&content_selector)
                .next()
                .map(|element| element.inner_html())
                .unwrap_or_default();
            
            Ok(html_escape::decode_html_entities(&content).into_owned())
        } else {
            // For URLs that don't match the expected format, return an empty string
            Ok(String::new())
        }
    }
}

#[async_trait]
impl ExchangeMonitor for BinanceMonitor {
    fn exchange_name(&self) -> &str {
        "Binance"
    }
    
    async fn fetch_announcements(&self) -> Result<Vec<Announcement>> {
        // First, check if the site is accessible
        tracing::info!("Attempting to fetch Binance announcements");
        
        // Request parameters for the Binance announcement API
        let params = serde_json::json!({
            "catalogId": "48",  // 48 is "New Crypto Listings"
            "pageNo": 1,
            "pageSize": 20,
        });
        
        // Prepare for retry logic with proxy rotation
        let headers = create_browser_headers(Some("application/json"), Some("www.binance.com"));
        let base_url_clone = self.base_url.clone();
        let params_clone = params.clone();
        
        // Use retry mechanism for the main request with proxy rotation
        match retry_request(
            move || {
                // Create a new client with different proxy for each retry attempt
                let client = create_new_proxy_client();
                let url = base_url_clone.clone();
                let headers = headers.clone();
                let params = params_clone.clone();
                async move {
                    client.post(&url)
                        .headers(headers)
                        .json(&params)
                        .send()
                        .await
                        .context("Failed to request Binance announcements")
                }
            },
            3, // max retries
            500, // initial delay in ms
        ).await {
            Ok(response) => {
                // Get response body for parsing
                let body = response.text().await.context("Failed to get Binance API response body")?;
                
                // Log the raw response for debugging
                tracing::debug!("Binance API response: {}", body);
                
                // Parse the response
                let binance_response: BinanceAnnouncementResponse = match serde_json::from_str(&body) {
                    Ok(resp) => resp,
                    Err(e) => {
                        tracing::error!(
                            exchange = self.exchange_name(),
                            error = %e,
                            "Failed to parse Binance API response. This could indicate a change in the API format."
                        );
                        
                        // If the body contains HTML, it's likely an error page
                        if body.contains("<html") || body.contains("<!DOCTYPE") {
                            tracing::error!("Received HTML response instead of JSON. API endpoint may have changed or access is blocked.");
                            // Return a more user-friendly error message
                            return Err(anyhow::anyhow!("Binance API returned HTML page instead of JSON data. The API endpoint may have changed or access may be restricted from your location."));
                        }
                        
                        return Err(anyhow::anyhow!("Failed to parse Binance announcement response: {}", e));
                    }
                };
                
                // Check if the request was successful
                if !binance_response.success {
                    return Err(anyhow::anyhow!(
                        "Binance API returned error: {}",
                        binance_response.message.unwrap_or_else(|| "Unknown error".to_string())
                    ));
                }
                
                // Convert Binance announcements to our standard format
                let mut announcements = Vec::new();
                for binance_announcement in binance_response.data {
                    // Only process if we have a URL
                    if let Some(url) = &binance_announcement.url {
                        // Convert timestamp to DateTime<Utc>
                        let published_at = DateTime::<Utc>::from_timestamp(
                            binance_announcement.release_date / 1000, // Convert milliseconds to seconds
                            0,
                        ).unwrap_or_else(|| Utc::now());
                        
                        // Clone the ID for use in error logging
                        let announcement_id = binance_announcement.id.clone();
                        
                        // Create the base announcement
                        let mut announcement = Announcement::new(
                            announcement_id.clone(), // Use the cloned ID so the original is still available for logging
                            binance_announcement.title,
                            String::new(), // We'll fetch content separately
                            url.clone(),
                            self.exchange_name().to_string(),
                            published_at,
                        );
                        
                        // Fetch the full content
                        match self.fetch_announcement_content(url).await {
                            Ok(content) => {
                                announcement.content = content;
                                // Analyze if this is a new listing
                                announcement.analyze_for_new_listing();
                                announcements.push(announcement);
                            }
                            Err(e) => {
                                tracing::warn!(
                                    exchange = self.exchange_name(),
                                    announcement_id = announcement_id,
                                    error = %e,
                                    "Failed to fetch content for Binance announcement"
                                );
                            }
                        }
                    }
                }
                
                Ok(announcements)
            },
            Err(e) => {
                let error_message = e.to_string();
                
                // Clean and enhance the error message
                let user_friendly_error = if error_message.contains("cloudfront") || error_message.contains("403 Forbidden") {
                    "Binance API access is currently blocked by CloudFront protection. This typically happens due to:
                    1. IP-based restrictions (your IP may be blocked)
                    2. Anti-bot measures detecting our automated requests
                    3. Geolocation restrictions
                    
                    Possible solutions:
                    1. Try using a proxy or VPN
                    2. Try again later as these blocks are often temporary
                    3. Consider using the official Binance API with authentication"
                } else {
                    &error_message
                };
                
                tracing::error!(
                    exchange = self.exchange_name(),
                    error = user_friendly_error,
                    "Failed to access Binance API"
                );
                
                Err(anyhow::anyhow!("{}", user_friendly_error))
            }
        }
    }
}
