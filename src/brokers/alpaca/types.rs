//! Alpaca-specific types for API responses
//!
//! These types map Alpaca's JSON responses to our domain types.
//! They are not exposed publicly - only our domain types are.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};

use crate::brokers::traits::{
    Account, Candle, CashDividendAnnouncement, CorporateActionAnnouncement, Order, OrderSide,
    OrderStatus, OrderType, Position, Quote, SplitAnnouncement, TimeInForce,
};
use crate::brokers::types::{OrderId, Price, Quantity, Symbol};
use crate::utils::errors::BrokerError;

// ============================================================================
// Alpaca API Response Types
// ============================================================================

/// Alpaca account response
#[derive(Debug, Clone, Deserialize)]
pub struct AlpacaAccount {
    pub account_number: String,
    pub buying_power: String,
    pub cash: String,
    pub portfolio_value: String,
    pub daytrade_count: u32,
    pub pattern_day_trader: bool,
    pub equity: String,
    pub last_equity: String,
}

impl AlpacaAccount {
    /// Converts to our domain Account type
    pub fn into_account(self) -> Result<Account, BrokerError> {
        Ok(Account {
            account_number: self.account_number,
            buying_power: parse_decimal(&self.buying_power)?,
            cash: parse_decimal(&self.cash)?,
            portfolio_value: parse_decimal(&self.portfolio_value)?,
            day_trade_count: self.daytrade_count,
            is_pattern_day_trader: self.pattern_day_trader,
            equity: parse_decimal(&self.equity)?,
            last_equity: parse_decimal(&self.last_equity)?,
        })
    }
}

/// Alpaca position response
#[derive(Debug, Clone, Deserialize)]
pub struct AlpacaPosition {
    pub symbol: String,
    pub qty: String,
    pub avg_entry_price: String,
    pub current_price: String,
    pub market_value: String,
    pub unrealized_pl: String,
    pub unrealized_plpc: String,
}

impl AlpacaPosition {
    /// Converts to our domain Position type
    pub fn into_position(self) -> Result<Position, BrokerError> {
        let symbol = Symbol::new(self.symbol)
            .map_err(|e| BrokerError::ApiError(format!("Invalid symbol: {}", e)))?;

        let qty_decimal = parse_decimal(&self.qty)?;
        let quantity = qty_decimal
            .to_string()
            .parse::<i32>()
            .map_err(|e| BrokerError::ApiError(format!("Invalid quantity: {}", e)))?;

        let avg_entry = parse_decimal(&self.avg_entry_price)?;
        let current = parse_decimal(&self.current_price)?;

        Ok(Position {
            symbol,
            quantity,
            avg_entry_price: Price::new(avg_entry)
                .map_err(|e| BrokerError::ApiError(format!("Invalid avg entry price: {}", e)))?,
            current_price: Price::new(current)
                .map_err(|e| BrokerError::ApiError(format!("Invalid current price: {}", e)))?,
            market_value: parse_decimal(&self.market_value)?,
            unrealized_pl: parse_decimal(&self.unrealized_pl)?,
            unrealized_pl_percent: parse_decimal(&self.unrealized_plpc)?,
        })
    }
}

/// Alpaca order response
#[derive(Debug, Clone, Deserialize)]
pub struct AlpacaOrder {
    pub id: String,
    pub symbol: String,
    pub side: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub qty: String,
    pub limit_price: Option<String>,
    pub stop_price: Option<String>,
    pub trail_percent: Option<String>,
    pub trail_price: Option<String>,
    pub status: String,
    pub time_in_force: String,
    pub filled_qty: String,
    pub filled_avg_price: Option<String>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub filled_at: Option<DateTime<Utc>>,
}

impl AlpacaOrder {
    /// Converts to our domain Order type
    pub fn into_order(self) -> Result<Order, BrokerError> {
        let symbol = Symbol::new(self.symbol)
            .map_err(|e| BrokerError::ApiError(format!("Invalid symbol: {}", e)))?;

        let qty = parse_decimal(&self.qty)?
            .to_string()
            .parse::<u32>()
            .map_err(|e| BrokerError::ApiError(format!("Invalid quantity: {}", e)))?;

        let filled_qty = parse_decimal(&self.filled_qty)?
            .to_string()
            .parse::<u32>()
            .map_err(|e| BrokerError::ApiError(format!("Invalid filled quantity: {}", e)))?;

        let limit_price = if let Some(lp) = self.limit_price {
            Some(
                Price::new(parse_decimal(&lp)?)
                    .map_err(|e| BrokerError::ApiError(format!("Invalid limit price: {}", e)))?,
            )
        } else {
            None
        };

        let stop_price = if let Some(sp) = self.stop_price {
            Some(
                Price::new(parse_decimal(&sp)?)
                    .map_err(|e| BrokerError::ApiError(format!("Invalid stop price: {}", e)))?,
            )
        } else {
            None
        };

        let filled_avg_price = if let Some(fap) = self.filled_avg_price {
            Some(
                Price::new(parse_decimal(&fap)?)
                    .map_err(|e| BrokerError::ApiError(format!("Invalid filled price: {}", e)))?,
            )
        } else {
            None
        };

        // Calculate trailing amount from trail_percent or trail_price
        let trailing_amount = if let Some(tp) = self.trail_percent {
            Some(parse_decimal(&tp)?)
        } else if let Some(tp) = self.trail_price {
            Some(parse_decimal(&tp)?)
        } else {
            None
        };

        Ok(Order {
            id: OrderId::new(self.id),
            symbol,
            side: parse_order_side(&self.side)?,
            order_type: parse_order_type(&self.order_type)?,
            quantity: Quantity::new(qty)
                .map_err(|e| BrokerError::ApiError(format!("Invalid quantity: {}", e)))?,
            limit_price,
            stop_price,
            trailing_amount,
            status: parse_order_status(&self.status)?,
            time_in_force: parse_time_in_force(&self.time_in_force)?,
            filled_quantity: filled_qty,
            filled_avg_price,
            submitted_at: self.submitted_at,
            filled_at: self.filled_at,
        })
    }
}

/// Alpaca quote message from WebSocket
#[derive(Debug, Clone, Deserialize)]
pub struct AlpacaQuote {
    #[serde(rename = "S")]
    pub symbol: String,
    #[serde(rename = "bp")]
    pub bid_price: f64,
    #[serde(rename = "ap")]
    pub ask_price: f64,
    #[serde(rename = "bs")]
    pub bid_size: u32,
    #[serde(rename = "as")]
    pub ask_size: u32,
    #[serde(rename = "t")]
    pub timestamp: DateTime<Utc>,
}

impl AlpacaQuote {
    /// Converts to our domain Quote type
    pub fn into_quote(self, last_price: Price) -> Result<Quote, BrokerError> {
        let symbol = Symbol::new(self.symbol)
            .map_err(|e| BrokerError::ApiError(format!("Invalid symbol: {}", e)))?;

        let bid = Price::new(
            Decimal::from_f64_retain(self.bid_price)
                .ok_or_else(|| BrokerError::ApiError("Invalid bid price".to_string()))?,
        )
        .map_err(|e| BrokerError::ApiError(format!("Invalid bid price: {}", e)))?;

        let ask = Price::new(
            Decimal::from_f64_retain(self.ask_price)
                .ok_or_else(|| BrokerError::ApiError("Invalid ask price".to_string()))?,
        )
        .map_err(|e| BrokerError::ApiError(format!("Invalid ask price: {}", e)))?;

        Ok(Quote {
            symbol,
            bid,
            ask,
            last: last_price,
            bid_size: self.bid_size,
            ask_size: self.ask_size,
            timestamp: self.timestamp,
        })
    }
}

/// Alpaca bar (candle) message from WebSocket
#[derive(Debug, Clone, Deserialize)]
pub struct AlpacaBar {
    #[serde(rename = "S")]
    pub symbol: String,
    #[serde(rename = "t")]
    pub timestamp: DateTime<Utc>,
    #[serde(rename = "o")]
    pub open: f64,
    #[serde(rename = "h")]
    pub high: f64,
    #[serde(rename = "l")]
    pub low: f64,
    #[serde(rename = "c")]
    pub close: f64,
    #[serde(rename = "v")]
    pub volume: u64,
}

impl AlpacaBar {
    /// Converts to our domain Candle type
    pub fn into_candle(self) -> Result<Candle, BrokerError> {
        let symbol = Symbol::new(self.symbol)
            .map_err(|e| BrokerError::ApiError(format!("Invalid symbol: {}", e)))?;

        let open = Price::new(
            Decimal::from_f64_retain(self.open)
                .ok_or_else(|| BrokerError::ApiError("Invalid open price".to_string()))?,
        )
        .map_err(|e| BrokerError::ApiError(format!("Invalid open price: {}", e)))?;

        let high = Price::new(
            Decimal::from_f64_retain(self.high)
                .ok_or_else(|| BrokerError::ApiError("Invalid high price".to_string()))?,
        )
        .map_err(|e| BrokerError::ApiError(format!("Invalid high price: {}", e)))?;

        let low = Price::new(
            Decimal::from_f64_retain(self.low)
                .ok_or_else(|| BrokerError::ApiError("Invalid low price".to_string()))?,
        )
        .map_err(|e| BrokerError::ApiError(format!("Invalid low price: {}", e)))?;

        let close = Price::new(
            Decimal::from_f64_retain(self.close)
                .ok_or_else(|| BrokerError::ApiError("Invalid close price".to_string()))?,
        )
        .map_err(|e| BrokerError::ApiError(format!("Invalid close price: {}", e)))?;

        Ok(Candle {
            symbol,
            timestamp: self.timestamp,
            open,
            high,
            low,
            close,
            volume: self.volume,
        })
    }
}

/// Bar from the historical bars REST endpoint (Market Data API v2)
///
/// Unlike the WebSocket [`AlpacaBar`], these carry no per-bar symbol — the
/// symbol lives on the [`AlpacaBarsResponse`] envelope.
#[derive(Debug, Clone, Deserialize)]
pub struct AlpacaHistoricalBar {
    #[serde(rename = "t")]
    pub timestamp: DateTime<Utc>,
    #[serde(rename = "o")]
    pub open: f64,
    #[serde(rename = "h")]
    pub high: f64,
    #[serde(rename = "l")]
    pub low: f64,
    #[serde(rename = "c")]
    pub close: f64,
    #[serde(rename = "v")]
    pub volume: u64,
}

impl AlpacaHistoricalBar {
    /// Converts to the domain Candle, attaching the requested symbol
    ///
    /// # Errors
    ///
    /// Rejects bars with an inverted range (high < low) or non-positive
    /// prices — external input is validated at this boundary.
    pub fn into_candle(self, symbol: &Symbol) -> Result<Candle, BrokerError> {
        if self.high < self.low {
            return Err(BrokerError::ApiError(format!(
                "Invalid bar for {}: high {} < low {}",
                symbol.as_str(),
                self.high,
                self.low
            )));
        }

        AlpacaBar {
            symbol: symbol.as_str().to_string(),
            timestamp: self.timestamp,
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            volume: self.volume,
        }
        .into_candle()
    }
}

/// Response envelope for `GET /v2/stocks/{symbol}/bars`
///
/// `bars` is `null` (not an empty array) when the window contains no data;
/// `next_page_token` drives pagination and is `null` on the last page.
#[derive(Debug, Deserialize)]
pub struct AlpacaBarsResponse {
    #[serde(default)]
    pub bars: Option<Vec<AlpacaHistoricalBar>>,
    #[serde(default)]
    pub next_page_token: Option<String>,
}

/// A single forward or reverse stock split from the Corporate Actions API.
///
/// # Verification status
///
/// This shape is implemented against Alpaca's documented Corporate Actions
/// data API and has **not** been verified against a live response — no
/// credentials were available in this environment. If a live call fails to
/// deserialize, this struct (field names, and the grouping in
/// [`AlpacaCorporateActionsByType`]) is the place to fix, alongside
/// [`RestClient::get_corporate_actions`](super::rest::RestClient::get_corporate_actions)'s
/// `#[ignore]`d live test.
#[derive(Debug, Clone, Deserialize)]
pub struct AlpacaSplit {
    pub symbol: String,
    pub ex_date: NaiveDate,
    /// Old shares in the ratio, as a decimal string (e.g. `"1"`)
    pub old_rate: String,
    /// New shares in the ratio, as a decimal string (e.g. `"4"`)
    pub new_rate: String,
}

impl AlpacaSplit {
    /// Converts to a domain [`CorporateActionAnnouncement`].
    fn into_announcement(self) -> Result<CorporateActionAnnouncement, BrokerError> {
        let from = integer_share_count(&self.symbol, "old_rate", &self.old_rate)?;
        let to = integer_share_count(&self.symbol, "new_rate", &self.new_rate)?;

        Ok(CorporateActionAnnouncement::Split(SplitAnnouncement {
            ex_date: self.ex_date,
            from,
            to,
        }))
    }
}

/// Parses a split ratio field (`old_rate`/`new_rate`) as a whole share
/// count.
///
/// `Decimal::to_u32` rounds rather than rejecting a fractional value (e.g.
/// `1.5` would silently become `2`), which would corrupt a split ratio
/// without any error — so the fractional part is checked explicitly first.
fn integer_share_count(symbol: &str, field: &str, raw: &str) -> Result<u32, BrokerError> {
    let value = parse_decimal(raw)?;
    if value.fract() != Decimal::ZERO {
        return Err(BrokerError::ApiError(format!(
            "non-integer split {field} for {symbol}: {raw}"
        )));
    }
    value.to_u32().ok_or_else(|| {
        BrokerError::ApiError(format!("split {field} out of range for {symbol}: {raw}"))
    })
}

/// A single cash dividend from the Corporate Actions API. See
/// [`AlpacaSplit`]'s verification-status caveat — it applies here too.
#[derive(Debug, Clone, Deserialize)]
pub struct AlpacaCashDividend {
    pub symbol: String,
    pub ex_date: NaiveDate,
    /// Cash amount per share, as a decimal string (e.g. `"0.24"`)
    pub rate: String,
}

impl AlpacaCashDividend {
    /// Converts to a domain [`CorporateActionAnnouncement`].
    fn into_announcement(self) -> Result<CorporateActionAnnouncement, BrokerError> {
        let amount = parse_decimal(&self.rate)?;
        Ok(CorporateActionAnnouncement::CashDividend(
            CashDividendAnnouncement {
                ex_date: self.ex_date,
                amount_per_share: amount,
            },
        ))
    }
}

/// Corporate actions grouped by type, as returned by the Corporate Actions
/// API. Alpaca groups announcements by type rather than returning a flat
/// list; only the two types this codebase models (forward/reverse splits,
/// cash dividends) are deserialized here — every other type (mergers,
/// spinoffs, name changes, ...) is simply absent from this struct, and
/// `#[serde(default)]` means its presence in a real response does not break
/// deserialization.
#[derive(Debug, Default, Deserialize)]
pub struct AlpacaCorporateActionsByType {
    #[serde(default)]
    pub forward_splits: Vec<AlpacaSplit>,
    #[serde(default)]
    pub reverse_splits: Vec<AlpacaSplit>,
    #[serde(default)]
    pub cash_dividends: Vec<AlpacaCashDividend>,
}

/// Response envelope for the Corporate Actions API.
///
/// See [`AlpacaSplit`]'s verification-status caveat.
#[derive(Debug, Default, Deserialize)]
pub struct AlpacaCorporateActionsResponse {
    #[serde(default)]
    pub corporate_actions: AlpacaCorporateActionsByType,
    #[serde(default)]
    pub next_page_token: Option<String>,
}

impl AlpacaCorporateActionsResponse {
    /// Flattens every modeled action across both split directions and cash
    /// dividends into domain announcements.
    ///
    /// # Errors
    ///
    /// Returns an error if any entry fails to convert (e.g. a non-integer
    /// split ratio).
    pub(crate) fn into_announcements(
        self,
    ) -> Result<Vec<CorporateActionAnnouncement>, BrokerError> {
        self.corporate_actions
            .forward_splits
            .into_iter()
            .chain(self.corporate_actions.reverse_splits)
            .map(AlpacaSplit::into_announcement)
            .chain(
                self.corporate_actions
                    .cash_dividends
                    .into_iter()
                    .map(AlpacaCashDividend::into_announcement),
            )
            .collect()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Parses a decimal from string
fn parse_decimal(s: &str) -> Result<Decimal, BrokerError> {
    s.parse::<Decimal>()
        .map_err(|e| BrokerError::ApiError(format!("Failed to parse decimal '{}': {}", s, e)))
}

/// Parses order side from Alpaca format
fn parse_order_side(s: &str) -> Result<OrderSide, BrokerError> {
    match s.to_lowercase().as_str() {
        "buy" => Ok(OrderSide::Buy),
        "sell" => Ok(OrderSide::Sell),
        _ => Err(BrokerError::ApiError(format!("Invalid order side: {}", s))),
    }
}

/// Parses order type from Alpaca format
fn parse_order_type(s: &str) -> Result<OrderType, BrokerError> {
    match s.to_lowercase().as_str() {
        "market" => Ok(OrderType::Market),
        "limit" => Ok(OrderType::Limit),
        "stop" => Ok(OrderType::Stop),
        "stop_limit" => Ok(OrderType::StopLimit),
        "trailing_stop" => Ok(OrderType::TrailingStop),
        _ => Err(BrokerError::ApiError(format!("Invalid order type: {}", s))),
    }
}

/// Parses order status from Alpaca format
fn parse_order_status(s: &str) -> Result<OrderStatus, BrokerError> {
    match s.to_lowercase().as_str() {
        "pending_new" | "new" | "accepted" => Ok(OrderStatus::Accepted),
        "partially_filled" => Ok(OrderStatus::PartiallyFilled),
        "filled" => Ok(OrderStatus::Filled),
        "canceled" | "cancelled" => Ok(OrderStatus::Cancelled),
        "rejected" => Ok(OrderStatus::Rejected),
        "expired" => Ok(OrderStatus::Expired),
        _ => Err(BrokerError::ApiError(format!(
            "Invalid order status: {}",
            s
        ))),
    }
}

/// Parses time in force from Alpaca format
fn parse_time_in_force(s: &str) -> Result<TimeInForce, BrokerError> {
    match s.to_lowercase().as_str() {
        "day" => Ok(TimeInForce::Day),
        "gtc" => Ok(TimeInForce::GTC),
        "ioc" => Ok(TimeInForce::IOC),
        "fok" => Ok(TimeInForce::FOK),
        _ => Err(BrokerError::ApiError(format!(
            "Invalid time in force: {}",
            s
        ))),
    }
}

/// Request body for placing an order
#[derive(Debug, Serialize)]
pub struct PlaceOrderRequest {
    pub symbol: String,
    pub qty: String,
    pub side: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub time_in_force: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trail_percent: Option<String>,
}

impl PlaceOrderRequest {
    /// Creates a request from our domain Order type
    pub fn from_order(order: &Order) -> Self {
        let side = match order.side {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        };

        let order_type = match order.order_type {
            OrderType::Market => "market",
            OrderType::Limit => "limit",
            OrderType::Stop => "stop",
            OrderType::StopLimit => "stop_limit",
            OrderType::TrailingStop => "trailing_stop",
        };

        let time_in_force = match order.time_in_force {
            TimeInForce::Day => "day",
            TimeInForce::GTC => "gtc",
            TimeInForce::IOC => "ioc",
            TimeInForce::FOK => "fok",
        };

        Self {
            symbol: order.symbol.as_str().to_string(),
            qty: order.quantity.value().to_string(),
            side: side.to_string(),
            order_type: order_type.to_string(),
            time_in_force: time_in_force.to_string(),
            limit_price: order.limit_price.map(|p| p.value().to_string()),
            stop_price: order.stop_price.map(|p| p.value().to_string()),
            trail_percent: order.trailing_amount.map(|t| t.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_order_side() {
        assert!(matches!(parse_order_side("buy").unwrap(), OrderSide::Buy));
        assert!(matches!(parse_order_side("sell").unwrap(), OrderSide::Sell));
        assert!(parse_order_side("invalid").is_err());
    }

    #[test]
    fn test_parse_order_type() {
        assert!(matches!(
            parse_order_type("market").unwrap(),
            OrderType::Market
        ));
        assert!(matches!(
            parse_order_type("limit").unwrap(),
            OrderType::Limit
        ));
        assert!(parse_order_type("invalid").is_err());
    }

    #[test]
    fn test_parse_order_status() {
        assert!(matches!(
            parse_order_status("new").unwrap(),
            OrderStatus::Accepted
        ));
        assert!(matches!(
            parse_order_status("filled").unwrap(),
            OrderStatus::Filled
        ));
        assert!(matches!(
            parse_order_status("canceled").unwrap(),
            OrderStatus::Cancelled
        ));
    }

    #[test]
    fn test_parse_decimal() {
        assert_eq!(parse_decimal("123.45").unwrap(), Decimal::new(12345, 2));
        assert!(parse_decimal("invalid").is_err());
    }

    #[test]
    fn test_bars_response_deserializes_page() {
        let json = r#"{
            "bars": [
                {"t": "2024-06-12T14:30:00Z", "o": 150.0, "h": 151.5, "l": 149.75, "c": 151.0, "v": 12345}
            ],
            "symbol": "AAPL",
            "next_page_token": "abc123"
        }"#;
        let response: AlpacaBarsResponse = serde_json::from_str(json).unwrap();
        let bars = response.bars.unwrap();
        assert_eq!(bars.len(), 1);
        assert_eq!(bars[0].volume, 12345);
        assert_eq!(response.next_page_token.as_deref(), Some("abc123"));
    }

    #[test]
    fn test_bars_response_deserializes_empty_window() {
        // Alpaca returns null (not []) for windows with no data
        let json = r#"{"bars": null, "symbol": "AAPL", "next_page_token": null}"#;
        let response: AlpacaBarsResponse = serde_json::from_str(json).unwrap();
        assert!(response.bars.is_none());
        assert!(response.next_page_token.is_none());
    }

    #[test]
    fn test_historical_bar_into_candle_valid() {
        let bar = AlpacaHistoricalBar {
            timestamp: "2024-06-12T14:30:00Z".parse().unwrap(),
            open: 150.0,
            high: 151.5,
            low: 149.75,
            close: 151.0,
            volume: 42,
        };
        let symbol = Symbol::new("AAPL").unwrap();
        let candle = bar.into_candle(&symbol).unwrap();
        assert_eq!(candle.symbol, symbol);
        assert_eq!(candle.volume, 42);
        assert_eq!(candle.high.as_f64(), 151.5);
    }

    #[test]
    fn test_historical_bar_into_candle_rejects_inverted_range() {
        let bar = AlpacaHistoricalBar {
            timestamp: "2024-06-12T14:30:00Z".parse().unwrap(),
            open: 150.0,
            high: 149.0, // below low
            low: 149.75,
            close: 151.0,
            volume: 42,
        };
        let result = bar.into_candle(&Symbol::new("AAPL").unwrap());
        assert!(matches!(result, Err(BrokerError::ApiError(_))));
    }

    #[test]
    fn test_corporate_actions_response_deserializes_documented_shape() {
        // Self-consistency check against the shape this module is
        // implemented against — not a live-verified fixture. See
        // AlpacaSplit's doc comment.
        let json = r#"{
            "corporate_actions": {
                "forward_splits": [
                    {"symbol": "AAPL", "ex_date": "2020-08-31", "old_rate": "1", "new_rate": "4"}
                ],
                "reverse_splits": [],
                "cash_dividends": [
                    {"symbol": "AAPL", "ex_date": "2020-05-08", "rate": "0.82"}
                ]
            },
            "next_page_token": null
        }"#;
        let response: AlpacaCorporateActionsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.corporate_actions.forward_splits.len(), 1);
        assert_eq!(response.corporate_actions.cash_dividends.len(), 1);
        assert!(response.next_page_token.is_none());
    }

    #[test]
    fn test_corporate_actions_response_ignores_unmodeled_types() {
        // Alpaca's real payload includes many more groups (mergers,
        // spinoffs, ...); this codebase only models splits and cash
        // dividends, and unknown keys must not break deserialization.
        let json = r#"{
            "corporate_actions": {
                "forward_splits": [],
                "reverse_splits": [],
                "cash_dividends": [],
                "cash_mergers": [{"symbol": "XYZ", "some_field": "value"}]
            },
            "next_page_token": null
        }"#;
        let response: AlpacaCorporateActionsResponse = serde_json::from_str(json).unwrap();
        assert!(response.corporate_actions.forward_splits.is_empty());
    }

    #[test]
    fn test_split_into_announcement_forward() {
        let split = AlpacaSplit {
            symbol: "AAPL".to_string(),
            ex_date: "2020-08-31".parse().unwrap(),
            old_rate: "1".to_string(),
            new_rate: "4".to_string(),
        };
        let announcement = split.into_announcement().unwrap();
        match announcement {
            CorporateActionAnnouncement::Split(s) => {
                assert_eq!(s.from, 1);
                assert_eq!(s.to, 4);
            }
            other => panic!("expected Split, got {other:?}"),
        }
    }

    #[test]
    fn test_split_into_announcement_rejects_non_integer_rate() {
        let split = AlpacaSplit {
            symbol: "AAPL".to_string(),
            ex_date: "2020-08-31".parse().unwrap(),
            old_rate: "1.5".to_string(),
            new_rate: "4".to_string(),
        };
        assert!(split.into_announcement().is_err());
    }

    #[test]
    fn test_cash_dividend_into_announcement() {
        let dividend = AlpacaCashDividend {
            symbol: "KO".to_string(),
            ex_date: "2024-03-15".parse().unwrap(),
            rate: "0.485".to_string(),
        };
        let announcement = dividend.into_announcement().unwrap();
        match announcement {
            CorporateActionAnnouncement::CashDividend(d) => {
                assert_eq!(d.amount_per_share, Decimal::new(485, 3));
            }
            other => panic!("expected CashDividend, got {other:?}"),
        }
    }

    #[test]
    fn test_corporate_actions_response_into_announcements_flattens_all_groups() {
        let response = AlpacaCorporateActionsResponse {
            corporate_actions: AlpacaCorporateActionsByType {
                forward_splits: vec![AlpacaSplit {
                    symbol: "AAPL".to_string(),
                    ex_date: "2020-08-31".parse().unwrap(),
                    old_rate: "1".to_string(),
                    new_rate: "4".to_string(),
                }],
                reverse_splits: vec![AlpacaSplit {
                    symbol: "XYZ".to_string(),
                    ex_date: "2024-01-15".parse().unwrap(),
                    old_rate: "10".to_string(),
                    new_rate: "1".to_string(),
                }],
                cash_dividends: vec![AlpacaCashDividend {
                    symbol: "KO".to_string(),
                    ex_date: "2024-03-15".parse().unwrap(),
                    rate: "0.485".to_string(),
                }],
            },
            next_page_token: None,
        };

        let announcements = response.into_announcements().unwrap();
        assert_eq!(announcements.len(), 3);
    }

    #[test]
    fn test_historical_bar_into_candle_rejects_non_positive_price() {
        let bar = AlpacaHistoricalBar {
            timestamp: "2024-06-12T14:30:00Z".parse().unwrap(),
            open: 0.0, // Price::new rejects non-positive values
            high: 151.5,
            low: 0.0,
            close: 151.0,
            volume: 42,
        };
        let result = bar.into_candle(&Symbol::new("AAPL").unwrap());
        assert!(matches!(result, Err(BrokerError::ApiError(_))));
    }
}
