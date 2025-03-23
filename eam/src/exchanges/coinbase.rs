use crate::exchanges::monitor::ExchangeMonitor;
use crate::models::announcement::Announcement;
use anyhow::{Result, Context};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use reqwest::Client;

/// Coinbase announcement monitor
pub struct CoinbaseMonitor {
    client: Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct CoinbaseBlogResponse {
    items: Vec<CoinbaseBlogPost>,
}

#[derive(Debug, Deserialize)]
struct CoinbaseBlogPost {
    id: String,
    title: String,
    #[serde(rename = "pubDate")]
    pub_date: String,
    link: String,
    content: String,
    #[serde(rename = "contentSnippet")]
    content_snippet: Option<String>,
    categories: Option<Vec<String>>,
}

impl CoinbaseMonitor {
    /// Create a new Coinbase monitor
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            // Using a RSS to JSON converter service for Coinbase blog
            base_url: "https://api.rss2json.com/v1/api.json?rss_url=https://blog.coinbase.com/feed".to_string(),
        }
    }
}

#[async_trait]
impl ExchangeMonitor for CoinbaseMonitor {
    fn exchange_name(&self) -> &str {
        "Coinbase"
    }
    
    async fn fetch_announcements(&self) -> Result<Vec<Announcement>> {
        // Make the API request
        let response = self.client.get(&self.base_url)
            .send()
            .await
            .context("Failed to request Coinbase blog RSS")?;
        
        // Parse the response
        let blog_response: CoinbaseBlogResponse = response.json()
            .await
            .context("Failed to parse Coinbase blog response")?;
        
        // Convert blog posts to our standard format
        let mut announcements = Vec::new();
        for blog_post in blog_response.items {
            // Parse publish time
            let published_at = DateTime::parse_from_rfc3339(&blog_post.pub_date)
                .unwrap_or_else(|_| Utc::now().into())
                .with_timezone(&Utc);
            
            // Get content from either full content or snippet
            let content = if !blog_post.content.is_empty() {
                blog_post.content
            } else {
                blog_post.content_snippet.unwrap_or_default()
            };
            
            // Create the announcement
            let mut announcement = Announcement::new(
                blog_post.id,
                blog_post.title,
                content,
                blog_post.link,
                self.exchange_name().to_string(),
                published_at,
            );
            
            // Analyze if this is a new listing
            announcement.analyze_for_new_listing();
            
            // If we have categories and they contain "listings" or similar keywords,
            // explicitly mark this as a new listing
            if let Some(categories) = blog_post.categories {
                let has_listing_category = categories.iter().any(|cat| {
                    let cat_lower = cat.to_lowercase();
                    cat_lower.contains("listing") || 
                    cat_lower.contains("new asset") || 
                    cat_lower.contains("new crypto")
                });
                
                if has_listing_category && !announcement.is_new_listing {
                    announcement.is_new_listing = true;
                    // Re-analyze for token symbols
                    announcement.analyze_for_new_listing();
                }
            }
            
            announcements.push(announcement);
        }
        
        Ok(announcements)
    }
}
