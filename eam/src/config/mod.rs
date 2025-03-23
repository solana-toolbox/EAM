use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::collections::HashMap;

/// Configuration for the exchange announcement monitoring application
#[derive(Debug, Clone)]
pub struct Config {
    /// Default interval in seconds between polling exchanges for new announcements
    pub default_polling_interval: u64,
    /// Exchange-specific polling intervals (if specified)
    pub exchange_intervals: HashMap<String, u64>,
    /// Enable monitoring for specific exchanges, or all if empty
    pub enabled_exchanges: Vec<String>,
    /// Log level
    pub log_level: String,
}

#[derive(Parser, Debug)]
#[command(name = "exchange-announcement-monitoring")]
#[command(about = "Monitor cryptocurrency exchange announcements for new token listings")]
pub struct CliArgs {
    /// Exchanges to monitor (comma-separated list)
    /// Leave empty to monitor all available exchanges
    #[arg(short, long, value_delimiter = ',')]
    pub exchanges: Vec<String>,
    
    /// Default interval in seconds between polling exchanges for announcements
    #[arg(short, long, default_value = "300")]
    pub interval: u64,
    
    /// Exchange-specific polling intervals in the format exchange:seconds
    /// Example: binance:60,okx:120
    #[arg(long, value_delimiter = ',')]
    pub exchange_intervals: Vec<String>,
    
    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    pub log_level: String,
    
    /// Path to dotenv file for configuration
    #[arg(long)]
    pub env_file: Option<PathBuf>,
}

impl Config {
    /// Create a new configuration from command line arguments and environment variables
    pub fn new() -> Result<Self> {
        let args = CliArgs::parse();
        
        // Load environment variables from .env file if specified
        if let Some(env_file) = &args.env_file {
            dotenv::from_path(env_file)?;
        } else {
            // Try to load from default .env file, but don't fail if not found
            let _ = dotenv::dotenv();
        }
        
        // Parse exchange-specific intervals
        let mut exchange_intervals = HashMap::new();
        for interval_str in &args.exchange_intervals {
            let parts: Vec<&str> = interval_str.split(':').collect();
            if parts.len() == 2 {
                if let Ok(seconds) = parts[1].parse::<u64>() {
                    exchange_intervals.insert(parts[0].to_string(), seconds);
                }
            }
        }
        
        Ok(Self {
            default_polling_interval: args.interval,
            exchange_intervals,
            enabled_exchanges: args.exchanges,
            log_level: args.log_level,
        })
    }
    
    /// Get the polling interval for a specific exchange
    pub fn get_polling_interval(&self, exchange_name: &str) -> u64 {
        self.exchange_intervals
            .get(exchange_name)
            .copied()
            .unwrap_or(self.default_polling_interval)
    }
    
    /// Check if an exchange should be monitored
    pub fn should_monitor_exchange(&self, exchange_name: &str) -> bool {
        self.enabled_exchanges.is_empty() || self.enabled_exchanges.contains(&exchange_name.to_string())
    }
}
