use anyhow::{Result, Context};
use futures::future;
use tokio::task::JoinSet;
use std::sync::Arc;

mod models;
mod exchanges;
mod config;
mod utils;

use crate::config::Config;
use crate::exchanges::monitor::ExchangeMonitor;
use crate::exchanges::{
    binance::BinanceMonitor,
    okx::OkxMonitor,
    bybit::BybitMonitor,
    bitmex::BitmexMonitor,
    gateio::GateioMonitor,
    kraken::KrakenMonitor,
    coinbase::CoinbaseMonitor,
    upbit::UpbitMonitor,
    bitget::BitgetMonitor,
    htx::HtxMonitor,
    mexc::MexcMonitor,
    kucoin::KucoinMonitor,
};

/// Create and return all available exchange monitors
fn create_exchange_monitors() -> Vec<Box<dyn ExchangeMonitor>> {
    vec![
        Box::new(BinanceMonitor::new()),
        Box::new(OkxMonitor::new()),
        Box::new(BybitMonitor::new()),
        Box::new(BitmexMonitor::new()),
        Box::new(GateioMonitor::new()),
        Box::new(KrakenMonitor::new()),
        Box::new(CoinbaseMonitor::new()),
        Box::new(UpbitMonitor::new()),
        Box::new(BitgetMonitor::new()),
        Box::new(HtxMonitor::new()),
        Box::new(MexcMonitor::new()),
        Box::new(KucoinMonitor::new()),
    ]
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = Config::new().context("Failed to load configuration")?;
    
    // Initialize logging
    utils::init_logger();
    
    tracing::info!("Starting Exchange Announcement Monitoring...");
    
    // Create exchange monitors
    let all_monitors = create_exchange_monitors();
    
    // Create a JoinSet to manage all the monitoring tasks
    let mut tasks = JoinSet::new();
    
    // Start monitoring for each enabled exchange
    for monitor in all_monitors {
        let exchange_name = monitor.exchange_name().to_string();
        
        // Check if we should monitor this exchange
        if !config.should_monitor_exchange(&exchange_name) {
            tracing::info!(exchange = exchange_name, "Skipping monitoring for {}", exchange_name);
            continue;
        }
        
        // Get the polling interval for this exchange
        let interval = config.get_polling_interval(&exchange_name);
        tracing::info!(
            exchange = exchange_name,
            interval_seconds = interval,
            "Starting monitor for {} with polling interval of {} seconds",
            exchange_name, interval
        );
        
        // Move the monitor into a thread-safe reference
        let monitor = Arc::new(monitor);
        
        // Spawn a task to run the monitor
        tasks.spawn(async move {
            let result = monitor.run(interval).await;
            if let Err(e) = result {
                tracing::error!(
                    exchange = exchange_name,
                    error = %e,
                    "Monitor for {} exited with error: {}",
                    exchange_name, e
                );
            }
            exchange_name
        });
    }
    
    // Wait for tasks to complete (which should not happen in normal operation)
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(exchange_name) => {
                tracing::error!(
                    exchange = exchange_name,
                    "Monitor for {} has unexpectedly terminated",
                    exchange_name
                );
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    "A monitor task panicked: {}",
                    e
                );
            }
        }
    }
    
    tracing::info!("All monitors have terminated. Exiting.");
    
    Ok(())
}
