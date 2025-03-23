# Exchange Announcement Monitoring

A Rust application that monitors cryptocurrency exchange announcements to detect new token listings and other important updates. The application supports multiple exchanges and runs monitors in parallel using separate threads.

## Features

- **Multi-Exchange Support**: Monitors announcements from 12 major cryptocurrency exchanges:
  - Binance
  - OKX
  - Bybit
  - BitMEX
  - Gate.io
  - Kraken
  - Coinbase
  - Upbit
  - Bitget
  - HTX (formerly Huobi)
  - MEXC
  - KuCoin

- **Multi-Threaded Architecture**: Each exchange is monitored in a separate thread for optimal performance and fault isolation.

- **New Token Listing Detection**: Automatically identifies announcements related to new token listings and extracts token symbols.

- **Configurable Polling Intervals**: Set default or exchange-specific polling intervals via command-line arguments or environment variables.

- **Structured Logging**: Uses `tracing` for comprehensive logging with different log levels and structured context.

- **Flexible Configuration**: Configure the application via command-line arguments or environment variables.

## Requirements

- Rust (2021 edition or newer)
- Internet connection to access exchange APIs

## Installation

### From Source

1. Clone the repository:
   ```bash
   git clone https://github.com/username/Exchange-Announcement-Monitoring.git
   cd Exchange-Announcement-Monitoring
   ```

2. Build the application:
   ```bash
   cargo build --release
   ```

3. The executable will be available at `target/release/exchange-announcement-monitoring`

## Configuration

The application can be configured in multiple ways:

### Command-Line Arguments

```
USAGE:
    exchange-announcement-monitoring [OPTIONS]

OPTIONS:
    -e, --exchanges <EXCHANGES>...           Exchanges to monitor (comma-separated list)
    -i, --interval <INTERVAL>                Default interval in seconds between polling [default: 300]
        --exchange-intervals <EXCHANGE_INTERVALS>...
                                             Exchange-specific polling intervals (format: exchange:seconds)
        --log-level <LOG_LEVEL>             Log level (trace, debug, info, warn, error) [default: info]
        --env-file <ENV_FILE>               Path to dotenv file for configuration
    -h, --help                               Print help information
```

### Environment Variables

You can also set configuration via environment variables:

- Create a `.env` file based on the provided `.env.example`
- Set the environment variables according to your needs

## Usage Examples

### Monitor All Exchanges with Default Settings

```bash
./exchange-announcement-monitoring
```

### Monitor Specific Exchanges

```bash
./exchange-announcement-monitoring --exchanges binance,okx,coinbase
```

### Set Custom Polling Intervals

```bash
./exchange-announcement-monitoring --interval 600 --exchange-intervals binance:180,coinbase:900
```

### Use a Custom Environment File

```bash
./exchange-announcement-monitoring --env-file ./custom-config.env
```

## How It Works

The application follows these key architectural principles:

1. **Exchange Monitor Interface**: Each exchange implements the `ExchangeMonitor` trait that defines a common interface for fetching and analyzing announcements.

2. **Standardized Announcement Format**: All exchange-specific announcement formats are converted to a standard `Announcement` model for consistent processing.

3. **Parallel Execution**: Each exchange monitor runs in its own asynchronous task, managed by Tokio's runtime.

4. **Non-blocking Operations**: All network requests and data processing are performed in a non-blocking manner to optimize performance.

5. **New Listing Detection**: The application analyzes announcement content to identify new token listings using keyword matching and pattern recognition.

## Error Handling

The application uses `anyhow` for comprehensive error handling:

- Exchange-specific errors are properly contextualized
- Network errors are handled gracefully with retries
- API response parsing errors are logged with detailed context

## Development

### Running Tests

```bash
cargo test
```

### Code Formatting

The project uses `rustfmt` for consistent code formatting:

```bash
cargo fmt
```

## License

MIT License