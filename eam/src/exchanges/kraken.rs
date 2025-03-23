use crate::exchanges::monitor::ExchangeMonitor;
use crate::models::announcement::Announcement;
use anyhow::{Result, Context};
use async_trait::async_trait;
use chrono::{DateTime, Utc, NaiveDateTime, TimeZone};
use reqwest::Client;
use scraper::{Html, Selector};

/// Kraken announcement monitor
pub struct KrakenMonitor {
    client: Client,
    base_url: String,
}

impl KrakenMonitor {
    /// Create a new Kraken monitor
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://blog.kraken.com/product-updates".to_string(),
        }
    }
    
    /// Parses a date string from Kraken's blog
    fn parse_date(&self, date_str: &str) -> DateTime<Utc> {
        // Example format: "May 15, 2023"
        if let Ok(naive_date) = NaiveDateTime::parse_from_str(&format!("{} 00:00:00", date_str), "%B %d, %Y %H:%M:%S") {
            Utc.from_utc_datetime(&naive_date)
        } else {
            tracing::warn!(
                exchange = self.exchange_name(),
                date_str = date_str,
                "Failed to parse Kraken date string"
            );
            Utc::now()
        }
    }
}

#[async_trait]
impl ExchangeMonitor for KrakenMonitor {
    fn exchange_name(&self) -> &str {
        "Kraken"
    }
    
    async fn fetch_announcements(&self) -> Result<Vec<Announcement>> {
        // Make the request to the Kraken blog
        let response = self.client.get(&self.base_url)
            .send()
            .await
            .context("Failed to request Kraken blog")?;
        
        let html = response.text()
            .await
            .context("Failed to get Kraken blog HTML")?;
        
        // Parse the HTML
        let document = Html::parse_document(&html);
        
        // Define selectors for blog posts
        let post_selector = Selector::parse("article.blog-post").unwrap();
        let title_selector = Selector::parse("h2.blog-post__title a").unwrap();
        let date_selector = Selector::parse("time.blog-post__date").unwrap();
        let excerpt_selector = Selector::parse("div.blog-post__excerpt").unwrap();
        
        let mut announcements = Vec::new();
        
        // Extract information from each blog post
        for post in document.select(&post_selector) {
            let title = post.select(&title_selector)
                .next()
                .map(|el| el.inner_html().trim().to_string())
                .unwrap_or_default();
            
            let url = post.select(&title_selector)
                .next()
                .and_then(|el| el.value().attr("href"))
                .map(|href| href.to_string())
                .unwrap_or_default();
            
            let date_str = post.select(&date_selector)
                .next()
                .map(|el| el.inner_html().trim().to_string())
                .unwrap_or_default();
            
            let content = post.select(&excerpt_selector)
                .next()
                .map(|el| el.inner_html().trim().to_string())
                .unwrap_or_default();
            
            // Skip if we don't have essential information
            if title.is_empty() || url.is_empty() {
                continue;
            }
            
            // Parse the date
            let published_at = self.parse_date(&date_str);
            
            // Generate a unique ID
            let id = format!("kraken_{}", url.replace("/", "_"));
            
            // Create the announcement
            let mut announcement = Announcement::new(
                id,
                title,
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
