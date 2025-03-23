use async_trait::async_trait;
use anyhow::Result;
use crate::models::announcement::Announcement;

/// ExchangeMonitor trait defines the common interface for all exchange announcement monitors
#[async_trait]
pub trait ExchangeMonitor: Send + Sync {
    /// Returns the name of the exchange being monitored
    fn exchange_name(&self) -> &str;
    
    /// Asynchronously fetches the latest announcements from the exchange
    async fn fetch_announcements(&self) -> Result<Vec<Announcement>>;
    
    /// Run the monitoring loop with the specified polling interval in seconds
    async fn run(&self, interval_seconds: u64) -> Result<()> {
        let exchange_name = self.exchange_name();
        
        tracing::info!(exchange = exchange_name, "Starting monitor for {}", exchange_name);
        
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval_seconds));
        
        loop {
            interval.tick().await;
            
            tracing::info!(exchange = exchange_name, "Fetching announcements from {}", exchange_name);
            
            match self.fetch_announcements().await {
                Ok(announcements) => {
                    let total = announcements.len();
                    let new_listings = announcements.iter()
                        .filter(|a| a.is_new_listing)
                        .count();
                    
                    tracing::info!(
                        exchange = exchange_name,
                        total_announcements = total,
                        new_listings = new_listings,
                        "Retrieved {} announcements from {}, {} are new listings",
                        total, exchange_name, new_listings
                    );
                    
                    // Process new listing announcements
                    for announcement in announcements.iter().filter(|a| a.is_new_listing) {
                        let token_list = announcement.token_symbols.join(", ");
                        tracing::info!(
                            exchange = exchange_name,
                            title = announcement.title,
                            tokens = token_list,
                            url = announcement.url,
                            "New listing announcement: {}",
                            announcement.title
                        );
                    }
                }
                Err(e) => {
                    tracing::error!(
                        exchange = exchange_name,
                        error = %e,
                        "Failed to fetch announcements from {}: {}",
                        exchange_name, e
                    );
                }
            }
        }
    }
}
