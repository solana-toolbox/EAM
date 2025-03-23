use anyhow::{Context, Result};
use reqwest::{header, Client, Response, StatusCode};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    sync::Arc,
    time::Duration,
    env,
};
use rand::Rng;
use lazy_static::lazy_static;
use tracing_subscriber::{
    fmt::format::FmtSpan,
    EnvFilter,
};
use tracing::{debug, warn};
use tokio;

lazy_static! {
    static ref PROXY_CONFIG: Option<Arc<ProxyConfig>> = ProxyConfig::from_env().map(Arc::new);
}

pub fn init_logger() {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_span_events(FmtSpan::CLOSE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");
}

pub fn create_browser_headers(
    content_type: Option<&str>,
    host: Option<&str>,
) -> header::HeaderMap {
    let mut headers = header::HeaderMap::new();
    
    // Common browser headers
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/116.0.0.0 Safari/537.36",
        ),
    );
    
    headers.insert(
        header::ACCEPT,
        header::HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7",
        ),
    );
    
    headers.insert(
        header::ACCEPT_LANGUAGE,
        header::HeaderValue::from_static("en-US,en;q=0.9"),
    );
    
    headers.insert(
        header::ACCEPT_ENCODING,
        header::HeaderValue::from_static("gzip, deflate, br"),
    );
    
    headers.insert(
        header::CONNECTION,
        header::HeaderValue::from_static("keep-alive"),
    );
    
    headers.insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("max-age=0"),
    );
    
    // Add content type if provided
    if let Some(content_type) = content_type {
        if let Ok(value) = header::HeaderValue::from_str(content_type) {
            headers.insert(header::CONTENT_TYPE, value);
        }
    }
    
    // Add host if provided
    if let Some(host) = host {
        if let Ok(value) = header::HeaderValue::from_str(host) {
            headers.insert(header::HOST, value);
        }
    }
    
    headers
}

#[derive(Debug)]
pub struct ProxyConfig {
    pub host: String,
    pub port_range: (u16, u16),
    pub system_proxy: Option<String>,
    current_index: AtomicUsize,
}

impl Clone for ProxyConfig {
    fn clone(&self) -> Self {
        ProxyConfig {
            host: self.host.clone(),
            port_range: self.port_range,
            system_proxy: self.system_proxy.clone(),
            current_index: AtomicUsize::new(self.current_index.load(Ordering::SeqCst)),
        }
    }
}

impl ProxyConfig {
    pub fn from_env() -> Option<Self> {
        let proxy_host = env::var("PROXY").ok()?;
        let port_range = env::var("PORT_RANGE").ok()?;
        let system_proxy = env::var("SYSTEM_PROXY").ok();
        
        // Parse port range in format "start-end"
        let parts: Vec<&str> = port_range.split('-').collect();
        if parts.len() != 2 {
            tracing::warn!("Invalid PORT_RANGE format. Expected 'start-end', got: {}", port_range);
            return None;
        }
        
        let start_port = parts[0].parse::<u16>().ok()?;
        let end_port = parts[1].parse::<u16>().ok()?;
        
        if start_port >= end_port {
            tracing::warn!("Invalid PORT_RANGE: start port must be less than end port");
            return None;
        }
        
        Some(ProxyConfig {
            host: proxy_host,
            port_range: (start_port, end_port),
            system_proxy,
            current_index: AtomicUsize::new(0),
        })
    }
    
    pub fn next_proxy_url(&self) -> String {
        let current = self.current_index.fetch_add(1, Ordering::SeqCst);
        let port_count = (self.port_range.1 - self.port_range.0) as usize + 1;
        let port = self.port_range.0 as usize + (current % port_count);
        
        format!("http://{}:{}", self.host, port)
    }
    
    pub fn random_proxy_url(&self) -> String {
        let port_count = (self.port_range.1 - self.port_range.0) as usize + 1;
        let random_index = rand::thread_rng().gen_range(0..port_count);
        let port = self.port_range.0 as usize + random_index;
        
        format!("http://{}:{}", self.host, port)
    }
}

/// Create a browser-like HTTP client
pub fn create_browser_client() -> Client {
    let builder = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/116.0.0.0 Safari/537.36");
    
    // Check if we have a proxy configuration and use it
    if let Some(proxy_config) = &*PROXY_CONFIG {
        if let Some(system_proxy) = &proxy_config.system_proxy {
            tracing::debug!("Using system proxy: {}", system_proxy);
            match reqwest::Proxy::all(system_proxy) {
                Ok(proxy) => {
                    return builder
                        .proxy(proxy)
                        .build()
                        .unwrap_or_else(|_| Client::new());
                }
                Err(e) => {
                    tracing::warn!("Failed to create system proxy: {}", e);
                }
            }
        }
        
        let proxy_url = proxy_config.next_proxy_url();
        tracing::debug!("Using proxy: {}", proxy_url);
        
        match reqwest::Proxy::all(&proxy_url) {
            Ok(proxy) => {
                return builder
                    .proxy(proxy)
                    .build()
                    .unwrap_or_else(|_| Client::new());
            }
            Err(e) => {
                tracing::warn!("Failed to create proxy: {}", e);
                return builder.build().unwrap_or_else(|_| Client::new());
            }
        }
    }
    
    builder.build().unwrap_or_else(|_| Client::new())
}

/// Create a new client with a random proxy from the configuration
pub fn create_new_proxy_client() -> Client {
    let builder = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/116.0.0.0 Safari/537.36");
    
    // Check if we have a proxy configuration and use it with a random port
    if let Some(proxy_config) = &*PROXY_CONFIG {
        if let Some(system_proxy) = &proxy_config.system_proxy {
            tracing::debug!("Using system proxy: {}", system_proxy);
            match reqwest::Proxy::all(system_proxy) {
                Ok(proxy) => {
                    return builder
                        .proxy(proxy)
                        .build()
                        .unwrap_or_else(|_| Client::new());
                }
                Err(e) => {
                    tracing::warn!("Failed to create system proxy: {}", e);
                }
            }
        }
        
        let proxy_url = proxy_config.random_proxy_url();
        tracing::debug!("Using random proxy: {}", proxy_url);
        
        match reqwest::Proxy::all(&proxy_url) {
            Ok(proxy) => {
                return builder
                    .proxy(proxy)
                    .build()
                    .unwrap_or_else(|_| Client::new());
            }
            Err(e) => {
                tracing::warn!("Failed to create random proxy: {}", e);
                return builder.build().unwrap_or_else(|_| Client::new());
            }
        }
    }
    
    builder.build().unwrap_or_else(|_| Client::new())
}

/// Creates a client with proxy support for HTTP requests
pub fn set_client_with_proxy() -> Result<Client> {
    Ok(create_new_proxy_client())
}

/// Retry a request with exponential backoff
/// 
/// This function will retry the request up to max_retries times, with an exponential
/// backoff starting at initial_delay_ms. Each retry will use a different proxy.
pub async fn retry_request<F, Fut>(
    request_fn: F,
    max_retries: usize,
    initial_delay_ms: u64,
) -> Result<Response>
where
    F: Fn() -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<Response>> + Send,
{
    let mut delay_ms = initial_delay_ms;
    let mut last_error = None;

    for attempt in 0..max_retries {
        match request_fn().await {
            Ok(response) => {
                if response.status().is_success() {
                    return Ok(response);
                } else {
                    let status = response.status();
                    if status == StatusCode::TOO_MANY_REQUESTS || status == StatusCode::FORBIDDEN {
                        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                        let error_msg = format!("Request failed with status {}: {}", status, error_text);
                        tracing::warn!("Attempt {} failed: {}", attempt + 1, error_msg);
                        last_error = Some(anyhow::anyhow!(error_msg));
                        
                        // CloudFront or rate limiting, wait with exponential backoff
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        delay_ms *= 2; // Exponential backoff
                        continue;
                    } else {
                        // For other errors, consider it a success and let the caller handle parsing
                        return Ok(response);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Attempt {} failed: {}", attempt + 1, e);
                last_error = Some(e);
                
                if attempt < max_retries - 1 {
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    delay_ms *= 2; // Exponential backoff
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Failed after {} attempts", max_retries)))
}

/// Extract data from a response, handling both JSON and HTML fallback
pub async fn extract_response_data<T>(response: Response, html_extractor: Option<fn(&str) -> Result<T>>) -> Result<T> 
where 
    T: serde::de::DeserializeOwned,
{
    let status = response.status();
    let content_type = response.headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string(); // Convert to owned string to avoid borrowing issues
    
    // Get the response body
    let body = response.text().await?;
    
    let is_html = content_type.contains("text/html");
    
    // Try to parse as JSON first
    let json_result = if is_html {
        tracing::warn!("Received HTML response when expecting JSON");
        Err(anyhow::anyhow!("Content-Type is HTML, not JSON"))
    } else {
        serde_json::from_str::<T>(&body).map_err(|e| anyhow::anyhow!("JSON parse error: {}", e))
    };
    
    match json_result {
        Ok(data) => Ok(data),
        Err(json_err) => {
            // If JSON parsing failed and we have HTML extractor function, try that
            if let Some(extractor) = html_extractor {
                tracing::info!("Trying HTML fallback extraction");
                extractor(&body)
            } else {
                // Debug the failing response
                if body.len() > 200 {
                    tracing::warn!("Response parsing failed. Status: {}, Content-Type: {}, Body start: {}", 
                        status, content_type, &body[..200]);
                } else {
                    tracing::warn!("Response parsing failed. Status: {}, Content-Type: {}, Body: {}", 
                        status, content_type, body);
                }
                Err(json_err)
            }
        }
    }
}
