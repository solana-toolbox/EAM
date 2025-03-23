use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Announcement represents a standardized format for exchange announcements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Announcement {
    /// Unique identifier for the announcement
    pub id: String,
    /// Title of the announcement
    pub title: String,
    /// Full text content of the announcement
    pub content: String,
    /// URL to the announcement source
    pub url: String,
    /// The exchange source of this announcement
    pub exchange: String,
    /// Publication date and time of the announcement
    pub published_at: DateTime<Utc>,
    /// Whether this announcement is about a new token listing
    pub is_new_listing: bool,
    /// If this is a new listing, the token symbol(s) mentioned
    pub token_symbols: Vec<String>,
}

impl Announcement {
    /// Creates a new Announcement instance
    pub fn new(
        id: String,
        title: String,
        content: String,
        url: String,
        exchange: String,
        published_at: DateTime<Utc>,
    ) -> Self {
        // Default is not a new listing
        let is_new_listing = false;
        let token_symbols = Vec::new();

        Self {
            id,
            title,
            content,
            url,
            exchange,
            published_at,
            is_new_listing,
            token_symbols,
        }
    }

    /// Analyzes the announcement content to determine if it's about a new token listing
    /// and extracts relevant token symbols
    pub fn analyze_for_new_listing(&mut self) {
        // Keywords that typically indicate a new token listing
        let listing_keywords = [
            "new listing", "listing", "new token", "new coin", "new cryptocurrency",
            "will list", "now available", "deposits open", "trading pairs", "添加", "上线",
        ];

        // Check if title or content contains listing keywords
        let title_lower = self.title.to_lowercase();
        let content_lower = self.content.to_lowercase();
        
        self.is_new_listing = listing_keywords.iter().any(|keyword| {
            title_lower.contains(keyword) || content_lower.contains(keyword)
        });

        // If this is a listing announcement, try to extract token symbols
        if self.is_new_listing {
            // This is a simplified approach - in reality you would use more sophisticated
            // NLP or pattern matching techniques to extract token symbols
            let mut symbols = Vec::new();
            
            // Look for patterns like "(BTC)" or "[ETH]" in the title and content
            let symbol_pattern = regex::Regex::new(r"[\(\[]([\w]{2,10})[\)\]]").unwrap();
            
            for cap in symbol_pattern.captures_iter(&self.title) {
                if let Some(symbol) = cap.get(1) {
                    symbols.push(symbol.as_str().to_uppercase());
                }
            }
            
            for cap in symbol_pattern.captures_iter(&self.content) {
                if let Some(symbol) = cap.get(1) {
                    let symbol = symbol.as_str().to_uppercase();
                    if !symbols.contains(&symbol) {
                        symbols.push(symbol);
                    }
                }
            }
            
            self.token_symbols = symbols.into_iter().map(String::from).collect();
        }
    }
}
