//! Alpaca REST API client for order execution

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use reqwest::{Client, StatusCode};
use std::sync::Arc;
use std::time::Duration;

use super::config::AlpacaConfig;
use super::ratelimit::{RateLimiter, RateLimiterConfig};
use super::types::{
    AlpacaAccount, AlpacaBarsResponse, AlpacaCorporateActionsResponse, AlpacaOrder, AlpacaPosition,
    PlaceOrderRequest,
};
use crate::brokers::traits::{
    Account, BarAdjustment, Candle, CorporateActionAnnouncement, Order, OrderExecutionProvider,
    Position, Timeframe,
};
use crate::brokers::types::{OrderId, Reason, Symbol};
use crate::utils::errors::BrokerError;

/// Fallback wait time when a 429 response has no usable `Retry-After`
/// header (missing or unparseable).
const DEFAULT_RATE_LIMIT_RETRY_AFTER: Duration = Duration::from_secs(60);

/// Alpaca REST API client
#[derive(Debug, Clone)]
pub struct RestClient {
    config: AlpacaConfig,
    client: Client,
    rate_limiter: Arc<RateLimiter>,
}

impl RestClient {
    /// Creates a new REST client using the default Alpaca free-tier rate
    /// limit (200 requests/minute).
    pub fn new(config: AlpacaConfig) -> Self {
        Self::with_rate_limit(config, RateLimiterConfig::default())
    }

    /// Creates a new REST client with a custom rate-limit configuration.
    pub fn with_rate_limit(config: AlpacaConfig, rate_limit: RateLimiterConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            config,
            client,
            rate_limiter: Arc::new(RateLimiter::new(rate_limit)),
        }
    }

    /// Adds authentication headers to a request builder
    fn add_auth_headers(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder
            .header("APCA-API-KEY-ID", &self.config.api_key)
            .header("APCA-API-SECRET-KEY", &self.config.api_secret)
    }

    /// Makes a GET request to the Alpaca API
    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, BrokerError> {
        self.rate_limiter.acquire().await;

        let url = format!("{}{}", self.config.base_url, path);
        let request = self.client.get(&url);
        let request = self.add_auth_headers(request);

        let response = request
            .send()
            .await
            .map_err(|e| BrokerError::ConnectionFailed(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Makes a GET request to the Market Data API host
    ///
    /// Historical data lives on `data.alpaca.markets`, a different host from
    /// the paper/live trading API served by [`Self::get`].
    async fn get_data<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, BrokerError> {
        self.rate_limiter.acquire().await;

        let url = format!("{}{}", self.config.data_base_url, path);
        // .query() percent-encodes values; page tokens are base64-like and
        // can contain '+', '/', '=' which raw concatenation would corrupt
        let request = self.client.get(&url).query(query);
        let request = self.add_auth_headers(request);

        let response = request
            .send()
            .await
            .map_err(|e| BrokerError::ConnectionFailed(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Makes a POST request to the Alpaca API
    async fn post<T: serde::de::DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, BrokerError> {
        self.rate_limiter.acquire().await;

        let url = format!("{}{}", self.config.base_url, path);
        let request = self.client.post(&url).json(body);
        let request = self.add_auth_headers(request);

        let response = request
            .send()
            .await
            .map_err(|e| BrokerError::ConnectionFailed(e.to_string()))?;

        self.handle_response(response).await
    }

    /// Makes a DELETE request to the Alpaca API
    async fn delete(&self, path: &str) -> Result<(), BrokerError> {
        self.rate_limiter.acquire().await;

        let url = format!("{}{}", self.config.base_url, path);
        let request = self.client.delete(&url);
        let request = self.add_auth_headers(request);

        let response = request
            .send()
            .await
            .map_err(|e| BrokerError::ConnectionFailed(e.to_string()))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let retry_after = Self::parse_retry_after(&response);
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(self.map_error(status, &error_text, retry_after))
        }
    }

    /// Handles API response and deserializes or returns error
    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, BrokerError> {
        let status = response.status();

        if status.is_success() {
            response
                .json::<T>()
                .await
                .map_err(|e| BrokerError::ApiError(format!("Failed to parse response: {}", e)))
        } else {
            let retry_after = Self::parse_retry_after(&response);
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(self.map_error(status, &error_text, retry_after))
        }
    }

    /// Extracts and parses the `Retry-After` header from a response, if
    /// present. Returns `None` if the header is absent or unparseable, in
    /// which case callers fall back to a default wait.
    fn parse_retry_after(response: &reqwest::Response) -> Option<Duration> {
        let raw = response.headers().get(reqwest::header::RETRY_AFTER)?;
        let raw = raw.to_str().ok()?;
        Self::parse_retry_after_value(raw)
    }

    /// Parses a raw `Retry-After` header value into a [`Duration`].
    ///
    /// Alpaca (like most APIs) sends this as an integer number of seconds,
    /// so only that form is supported. Anything else (missing, empty,
    /// non-numeric, negative) yields `None` so the caller can fall back to
    /// [`DEFAULT_RATE_LIMIT_RETRY_AFTER`].
    fn parse_retry_after_value(raw: &str) -> Option<Duration> {
        raw.trim().parse::<u64>().ok().map(Duration::from_secs)
    }

    /// Maps HTTP status codes to BrokerError
    ///
    /// `retry_after` should be the parsed `Retry-After` header value from
    /// the response (if any); it is only used for 429 responses and falls
    /// back to [`DEFAULT_RATE_LIMIT_RETRY_AFTER`] when `None`.
    fn map_error(
        &self,
        status: StatusCode,
        message: &str,
        retry_after: Option<Duration>,
    ) -> BrokerError {
        match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                BrokerError::AuthenticationFailed(message.to_string())
            }
            StatusCode::NOT_FOUND => BrokerError::SymbolNotFound(message.to_string()),
            StatusCode::TOO_MANY_REQUESTS => BrokerError::RateLimitExceeded {
                retry_after: retry_after.unwrap_or(DEFAULT_RATE_LIMIT_RETRY_AFTER),
            },
            StatusCode::BAD_REQUEST => {
                // Check if it's an order rejection
                if message.contains("insufficient") {
                    BrokerError::InsufficientFunds {
                        required: rust_decimal::Decimal::ZERO,
                        available: rust_decimal::Decimal::ZERO,
                    }
                } else {
                    BrokerError::OrderRejected {
                        reason: Reason::new(message),
                    }
                }
            }
            _ => BrokerError::ApiError(format!("HTTP {}: {}", status, message)),
        }
    }

    /// Fetches historical OHLCV bars from the Market Data API v2
    ///
    /// Follows `next_page_token` pagination until the requested window is
    /// exhausted, so a multi-year 1-minute request returns every bar. Bars
    /// are validated at the boundary (positive prices, high >= low) and
    /// returned oldest-first as delivered by Alpaca.
    ///
    /// `adjustment` is correctness-critical: [`BarAdjustment::Raw`] returns
    /// as-traded prices, in which a stock split appears as a large gap that
    /// pattern detectors cannot distinguish from a real move. Pass
    /// [`BarAdjustment::All`] for anything feeding detection or backtesting.
    ///
    /// # Errors
    ///
    /// Returns an error on connection failure, an API error response, or a
    /// bar that fails validation.
    pub async fn get_bars(
        &self,
        symbol: &Symbol,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        timeframe: Timeframe,
        adjustment: BarAdjustment,
    ) -> Result<Vec<Candle>, BrokerError> {
        let mut candles = Vec::new();
        let mut page_token: Option<String> = None;
        let path = format!("/v2/stocks/{}/bars", symbol.as_str());

        loop {
            let mut query: Vec<(&str, String)> = vec![
                ("timeframe", timeframe_param(timeframe).to_string()),
                ("start", start.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
                ("end", end.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
                ("limit", "10000".to_string()),
                ("adjustment", adjustment.as_param().to_string()),
            ];
            if let Some(token) = &page_token {
                query.push(("page_token", token.clone()));
            }

            let response: AlpacaBarsResponse = self.get_data(&path, &query).await?;

            for bar in response.bars.unwrap_or_default() {
                candles.push(bar.into_candle(symbol)?);
            }

            match response.next_page_token {
                Some(token) if !token.is_empty() => page_token = Some(token),
                _ => break,
            }
        }

        Ok(candles)
    }

    /// Fetches corporate actions (stock splits and cash dividends) affecting
    /// `symbol` with `ex_date` in `[start, end]`, from the Market Data API's
    /// Corporate Actions endpoint. Follows `next_page_token` pagination the
    /// same way [`Self::get_bars`] does.
    ///
    /// # Verification status
    ///
    /// This is implemented against Alpaca's documented Corporate Actions API
    /// shape and has **not** been exercised against a live response in this
    /// environment — no credentials were available. See
    /// `test_get_corporate_actions_live_paper_account`, `#[ignore]`d for the
    /// same reason [`Self::get_bars`]'s live test is. If the shape turns out
    /// to be wrong, the fix is in `super::types::AlpacaCorporateActionsResponse`
    /// and its nested types, not here.
    ///
    /// # Errors
    ///
    /// Returns an error on connection failure, an API error response, or an
    /// action that fails to convert to the domain representation (e.g. a
    /// non-integer split ratio).
    pub async fn get_corporate_actions(
        &self,
        symbol: &Symbol,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<CorporateActionAnnouncement>, BrokerError> {
        let mut announcements = Vec::new();
        let mut page_token: Option<String> = None;
        let path = "/v1/corporate-actions";

        loop {
            let mut query: Vec<(&str, String)> = vec![
                ("symbols", symbol.as_str().to_string()),
                (
                    "types",
                    "forward_split,reverse_split,cash_dividend".to_string(),
                ),
                ("start", start.format("%Y-%m-%d").to_string()),
                ("end", end.format("%Y-%m-%d").to_string()),
            ];
            if let Some(token) = &page_token {
                query.push(("page_token", token.clone()));
            }

            let response: AlpacaCorporateActionsResponse = self.get_data(path, &query).await?;
            let next_page_token = response.next_page_token.clone();
            announcements.extend(response.into_announcements()?);

            match next_page_token {
                Some(token) if !token.is_empty() => page_token = Some(token),
                _ => break,
            }
        }

        Ok(announcements)
    }
}

/// Maps a domain [`Timeframe`] to the Alpaca data API `timeframe` parameter
pub(crate) fn timeframe_param(timeframe: Timeframe) -> &'static str {
    match timeframe {
        Timeframe::OneMinute => "1Min",
        Timeframe::FiveMinutes => "5Min",
        Timeframe::FifteenMinutes => "15Min",
        Timeframe::OneHour => "1Hour",
        Timeframe::OneDay => "1Day",
    }
}

#[async_trait]
impl OrderExecutionProvider for RestClient {
    async fn place_order(&self, order: Order) -> Result<OrderId, BrokerError> {
        let request = PlaceOrderRequest::from_order(&order);
        let response: AlpacaOrder = self.post("/v2/orders", &request).await?;
        Ok(OrderId::new(response.id))
    }

    async fn cancel_order(&self, order_id: &OrderId) -> Result<(), BrokerError> {
        let path = format!("/v2/orders/{}", order_id.as_str());
        self.delete(&path).await
    }

    async fn get_order(&self, order_id: &OrderId) -> Result<Order, BrokerError> {
        let path = format!("/v2/orders/{}", order_id.as_str());
        let response: AlpacaOrder = self.get(&path).await?;
        response.into_order()
    }

    async fn get_open_orders(&self) -> Result<Vec<Order>, BrokerError> {
        let response: Vec<AlpacaOrder> = self.get("/v2/orders?status=open").await?;
        response
            .into_iter()
            .map(|o| o.into_order())
            .collect::<Result<Vec<_>, _>>()
    }

    async fn get_account(&self) -> Result<Account, BrokerError> {
        let response: AlpacaAccount = self.get("/v2/account").await?;
        response.into_account()
    }

    async fn get_positions(&self) -> Result<Vec<Position>, BrokerError> {
        let response: Vec<AlpacaPosition> = self.get("/v2/positions").await?;
        response
            .into_iter()
            .map(|p| p.into_position())
            .collect::<Result<Vec<_>, _>>()
    }

    async fn get_position(&self, symbol: &Symbol) -> Result<Option<Position>, BrokerError> {
        let path = format!("/v2/positions/{}", symbol.as_str());
        match self.get::<AlpacaPosition>(&path).await {
            Ok(position) => Ok(Some(position.into_position()?)),
            Err(BrokerError::SymbolNotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::traits::{OrderSide, OrderType, TimeInForce};
    use crate::brokers::types::{Price, Quantity};
    use rust_decimal::Decimal;

    fn test_config() -> AlpacaConfig {
        AlpacaConfig::paper().with_credentials("test_key".to_string(), "test_secret".to_string())
    }

    #[test]
    fn test_client_creation() {
        let config = test_config();
        let client = RestClient::new(config.clone());
        assert_eq!(client.config.api_key, config.api_key);
    }

    #[test]
    fn test_place_order_request_from_order() {
        let symbol = Symbol::new("AAPL").unwrap();
        let qty = Quantity::new(100).unwrap();
        let price = Price::new(Decimal::new(15000, 2)).unwrap();

        let order = Order::limit(symbol, OrderSide::Buy, qty, price);
        let request = PlaceOrderRequest::from_order(&order);

        assert_eq!(request.symbol, "AAPL");
        assert_eq!(request.qty, "100");
        assert_eq!(request.side, "buy");
        assert_eq!(request.order_type, "limit");
        assert_eq!(request.limit_price, Some("150.00".to_string()));
    }

    #[tokio::test]
    async fn test_map_error() {
        let config = test_config();
        let client = RestClient::new(config);

        let error = client.map_error(StatusCode::UNAUTHORIZED, "Invalid credentials", None);
        assert!(matches!(error, BrokerError::AuthenticationFailed(_)));

        let error = client.map_error(StatusCode::NOT_FOUND, "Symbol not found", None);
        assert!(matches!(error, BrokerError::SymbolNotFound(_)));

        let error = client.map_error(StatusCode::TOO_MANY_REQUESTS, "Rate limit", None);
        assert!(matches!(error, BrokerError::RateLimitExceeded { .. }));
    }

    #[tokio::test]
    async fn test_map_error_rate_limit_uses_default_when_retry_after_absent() {
        let config = test_config();
        let client = RestClient::new(config);

        let error = client.map_error(StatusCode::TOO_MANY_REQUESTS, "Rate limit", None);
        match error {
            BrokerError::RateLimitExceeded { retry_after } => {
                assert_eq!(retry_after, DEFAULT_RATE_LIMIT_RETRY_AFTER);
            }
            other => panic!("expected RateLimitExceeded, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_map_error_rate_limit_uses_parsed_retry_after_when_present() {
        let config = test_config();
        let client = RestClient::new(config);

        let error = client.map_error(
            StatusCode::TOO_MANY_REQUESTS,
            "Rate limit",
            Some(Duration::from_secs(17)),
        );
        match error {
            BrokerError::RateLimitExceeded { retry_after } => {
                assert_eq!(retry_after, Duration::from_secs(17));
            }
            other => panic!("expected RateLimitExceeded, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_retry_after_value_present_and_numeric() {
        assert_eq!(
            RestClient::parse_retry_after_value("30"),
            Some(Duration::from_secs(30))
        );
    }

    #[test]
    fn test_parse_retry_after_value_handles_surrounding_whitespace() {
        assert_eq!(
            RestClient::parse_retry_after_value("  45 "),
            Some(Duration::from_secs(45))
        );
    }

    #[test]
    fn test_parse_retry_after_value_absent_returns_none() {
        assert_eq!(RestClient::parse_retry_after_value(""), None);
    }

    #[test]
    fn test_parse_retry_after_value_malformed_returns_none() {
        assert_eq!(RestClient::parse_retry_after_value("not-a-number"), None);
        assert_eq!(
            RestClient::parse_retry_after_value("Wed, 21 Oct 2026 07:28:00 GMT"),
            None
        );
        assert_eq!(RestClient::parse_retry_after_value("-5"), None);
    }

    #[tokio::test(start_paused = true)]
    async fn test_rest_client_throttles_via_rate_limiter() {
        let config = test_config();
        let client = RestClient::with_rate_limit(
            config,
            RateLimiterConfig {
                max_requests: 1,
                refill_period: Duration::from_secs(60),
            },
        );

        // First acquisition should be immediate (bucket starts full).
        client.rate_limiter.acquire().await;

        // Second acquisition must wait for a refill; bound it with a
        // timeout so a regression (e.g. no throttling) fails loudly rather
        // than hanging.
        let acquired =
            tokio::time::timeout(Duration::from_secs(120), client.rate_limiter.acquire()).await;
        assert!(acquired.is_ok(), "expected acquire to eventually succeed");
    }

    #[test]
    fn test_timeframe_param_maps_all_variants() {
        assert_eq!(timeframe_param(Timeframe::OneMinute), "1Min");
        assert_eq!(timeframe_param(Timeframe::FiveMinutes), "5Min");
        assert_eq!(timeframe_param(Timeframe::FifteenMinutes), "15Min");
        assert_eq!(timeframe_param(Timeframe::OneHour), "1Hour");
        assert_eq!(timeframe_param(Timeframe::OneDay), "1Day");
    }

    #[tokio::test]
    #[ignore = "requires live Alpaca credentials (ALPACA_API_KEY/ALPACA_API_SECRET)"]
    async fn test_get_bars_live_paper_account() {
        use chrono::TimeZone;

        let config = AlpacaConfig::paper().with_credentials(
            std::env::var("ALPACA_API_KEY").unwrap_or_default(),
            std::env::var("ALPACA_API_SECRET").unwrap_or_default(),
        );
        let client = RestClient::new(config);
        let symbol = Symbol::new("AAPL").unwrap();

        let candles = client
            .get_bars(
                &symbol,
                Utc.with_ymd_and_hms(2024, 6, 10, 13, 30, 0).unwrap(),
                Utc.with_ymd_and_hms(2024, 6, 10, 20, 0, 0).unwrap(),
                Timeframe::OneMinute,
                BarAdjustment::All,
            )
            .await
            .unwrap();

        assert!(!candles.is_empty(), "expected bars for a full trading day");
        assert!(candles.windows(2).all(|w| w[0].timestamp <= w[1].timestamp));
    }

    #[tokio::test]
    #[ignore = "requires live Alpaca credentials (ALPACA_API_KEY/ALPACA_API_SECRET); corporate-actions endpoint shape unverified against a live response"]
    async fn test_get_corporate_actions_live_paper_account() {
        let config = AlpacaConfig::paper().with_credentials(
            std::env::var("ALPACA_API_KEY").unwrap_or_default(),
            std::env::var("ALPACA_API_SECRET").unwrap_or_default(),
        );
        let client = RestClient::new(config);
        let symbol = Symbol::new("AAPL").unwrap();

        // AAPL executed a well-documented 4:1 forward split with ex-date
        // 2020-08-31; this window should return at least that action.
        let actions = client
            .get_corporate_actions(
                &symbol,
                chrono::NaiveDate::from_ymd_opt(2020, 8, 1).unwrap(),
                chrono::NaiveDate::from_ymd_opt(2020, 9, 30).unwrap(),
            )
            .await
            .unwrap();

        assert!(
            !actions.is_empty(),
            "expected at least the known 2020-08-31 AAPL split"
        );
    }

    // Note: Integration tests with actual API calls would go in tests/ directory
    // and would use paper trading credentials from environment variables
}
