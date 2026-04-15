//! Alpaca WebSocket client for real-time market data

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio::time::{Duration, interval};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::config::AlpacaConfig;
use super::types::{AlpacaBar, AlpacaQuote};
use crate::brokers::traits::{Candle, MarketDataProvider, Quote, Timeframe};
use crate::brokers::types::{Price, Symbol};
use crate::utils::errors::BrokerError;

/// WebSocket message types from Alpaca
#[derive(Debug, Deserialize)]
#[serde(tag = "T")]
enum WsMessage {
    /// Connection success
    #[serde(rename = "success")]
    Success { msg: String },

    /// Subscription confirmation
    #[serde(rename = "subscription")]
    Subscription {
        trades: Vec<String>,
        quotes: Vec<String>,
        bars: Vec<String>,
    },

    /// Error message
    #[serde(rename = "error")]
    Error { msg: String },

    /// Quote update
    #[serde(rename = "q")]
    Quote(AlpacaQuote),

    /// Bar/candle update
    #[serde(rename = "b")]
    Bar(AlpacaBar),
}

/// Authentication message for WebSocket
#[derive(Debug, Serialize)]
struct AuthMessage {
    action: String,
    key: String,
    secret: String,
}

/// Subscribe message for WebSocket
#[derive(Debug, Serialize)]
struct SubscribeMessage {
    action: String,
    quotes: Vec<String>,
    bars: Vec<String>,
}

/// Internal state for WebSocket client
struct WsState {
    /// Currently subscribed symbols
    subscribed_symbols: HashMap<String, bool>,
    /// Latest quotes by symbol
    latest_quotes: HashMap<String, Quote>,
    /// Latest bars by symbol
    latest_bars: HashMap<String, Candle>,
    /// Whether the stream is active
    is_streaming: bool,
}

/// Alpaca WebSocket client
pub struct WebSocketClient {
    config: AlpacaConfig,
    state: Arc<RwLock<WsState>>,
    command_tx: mpsc::UnboundedSender<WsCommand>,
}

/// Commands for WebSocket client
enum WsCommand {
    Subscribe(Vec<Symbol>),
    Unsubscribe(Vec<Symbol>),
    Stop,
}

impl WebSocketClient {
    /// Connects to Alpaca WebSocket and starts the client
    pub async fn connect(config: &AlpacaConfig) -> Result<Self, BrokerError> {
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        let state = Arc::new(RwLock::new(WsState {
            subscribed_symbols: HashMap::new(),
            latest_quotes: HashMap::new(),
            latest_bars: HashMap::new(),
            is_streaming: false,
        }));

        let client = Self {
            config: config.clone(),
            state: state.clone(),
            command_tx,
        };

        // Spawn the WebSocket handler task
        tokio::spawn(Self::ws_handler(config.clone(), state, command_rx));

        Ok(client)
    }

    /// Main WebSocket handler loop
    async fn ws_handler(
        config: AlpacaConfig,
        state: Arc<RwLock<WsState>>,
        mut command_rx: mpsc::UnboundedReceiver<WsCommand>,
    ) {
        loop {
            match Self::connect_and_run(&config, &state, &mut command_rx).await {
                Ok(_) => {
                    log::info!("WebSocket connection closed normally");
                    break;
                }
                Err(e) => {
                    log::error!("WebSocket error: {}, reconnecting in 5s...", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    /// Connects to WebSocket and runs until disconnection
    async fn connect_and_run(
        config: &AlpacaConfig,
        state: &Arc<RwLock<WsState>>,
        command_rx: &mut mpsc::UnboundedReceiver<WsCommand>,
    ) -> Result<(), BrokerError> {
        // Connect to WebSocket
        let (ws_stream, _) = connect_async(&config.ws_url)
            .await
            .map_err(|e| BrokerError::ConnectionFailed(e.to_string()))?;

        let (mut write, mut read) = ws_stream.split();

        // Authenticate
        let auth_msg = AuthMessage {
            action: "auth".to_string(),
            key: config.api_key.clone(),
            secret: config.api_secret.clone(),
        };
        let auth_json =
            serde_json::to_string(&auth_msg).map_err(|e| BrokerError::ApiError(e.to_string()))?;
        write
            .send(Message::Text(auth_json))
            .await
            .map_err(|e| BrokerError::ConnectionFailed(e.to_string()))?;

        // Set streaming state
        {
            let mut state_lock = state.write().await;
            state_lock.is_streaming = true;
        }

        // Heartbeat interval
        let mut heartbeat = interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                // Handle incoming WebSocket messages
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            Self::handle_message(&text, state).await;
                        }
                        Some(Ok(Message::Close(_))) => {
                            log::info!("WebSocket closed by server");
                            break;
                        }
                        Some(Err(e)) => {
                            log::error!("WebSocket read error: {}", e);
                            break;
                        }
                        None => {
                            log::info!("WebSocket stream ended");
                            break;
                        }
                        _ => {}
                    }
                }

                // Handle commands from client
                cmd = command_rx.recv() => {
                    match cmd {
                        Some(WsCommand::Subscribe(symbols)) => {
                            Self::send_subscribe(&mut write, &symbols, state).await?;
                        }
                        Some(WsCommand::Unsubscribe(symbols)) => {
                            Self::send_unsubscribe(&mut write, &symbols, state).await?;
                        }
                        Some(WsCommand::Stop) | None => {
                            log::info!("Stop command received");
                            break;
                        }
                    }
                }

                // Send periodic heartbeat
                _ = heartbeat.tick() => {
                    if write.send(Message::Ping(vec![])).await.is_err() {
                        log::error!("Failed to send heartbeat");
                        break;
                    }
                }
            }
        }

        // Clear streaming state
        {
            let mut state_lock = state.write().await;
            state_lock.is_streaming = false;
        }

        Ok(())
    }

    /// Handles incoming WebSocket message
    async fn handle_message(text: &str, state: &Arc<RwLock<WsState>>) {
        // Try to parse as array of messages
        if let Ok(messages) = serde_json::from_str::<Vec<WsMessage>>(text) {
            for msg in messages {
                Self::process_message(msg, state).await;
            }
        } else if let Ok(msg) = serde_json::from_str::<WsMessage>(text) {
            Self::process_message(msg, state).await;
        } else {
            log::warn!("Failed to parse WebSocket message: {}", text);
        }
    }

    /// Processes a single WebSocket message
    async fn process_message(msg: WsMessage, state: &Arc<RwLock<WsState>>) {
        match msg {
            WsMessage::Success { msg } => {
                log::info!("WebSocket success: {}", msg);
            }
            WsMessage::Subscription { quotes, bars, .. } => {
                log::info!("Subscribed to quotes: {:?}, bars: {:?}", quotes, bars);
            }
            WsMessage::Error { msg } => {
                log::error!("WebSocket error: {}", msg);
            }
            WsMessage::Quote(alpaca_quote) => {
                // Get last price from latest bar or use ask price as fallback
                let last_price = {
                    let state_lock = state.read().await;
                    state_lock
                        .latest_bars
                        .get(&alpaca_quote.symbol)
                        .map(|bar| bar.close)
                        .or_else(|| {
                            Price::new(
                                rust_decimal::Decimal::from_f64_retain(alpaca_quote.ask_price)
                                    .unwrap_or_default(),
                            )
                            .ok()
                        })
                };

                if let Some(last) = last_price {
                    if let Ok(quote) = alpaca_quote.into_quote(last) {
                        let mut state_lock = state.write().await;
                        state_lock
                            .latest_quotes
                            .insert(quote.symbol.as_str().to_string(), quote);
                    }
                }
            }
            WsMessage::Bar(alpaca_bar) => {
                if let Ok(candle) = alpaca_bar.into_candle() {
                    let mut state_lock = state.write().await;
                    state_lock
                        .latest_bars
                        .insert(candle.symbol.as_str().to_string(), candle);
                }
            }
        }
    }

    /// Sends subscribe message
    async fn send_subscribe(
        write: &mut futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            Message,
        >,
        symbols: &[Symbol],
        state: &Arc<RwLock<WsState>>,
    ) -> Result<(), BrokerError> {
        let symbol_strs: Vec<String> = symbols.iter().map(|s| s.as_str().to_string()).collect();

        let sub_msg = SubscribeMessage {
            action: "subscribe".to_string(),
            quotes: symbol_strs.clone(),
            bars: symbol_strs.clone(),
        };

        let json =
            serde_json::to_string(&sub_msg).map_err(|e| BrokerError::ApiError(e.to_string()))?;

        write
            .send(Message::Text(json))
            .await
            .map_err(|e| BrokerError::ConnectionFailed(e.to_string()))?;

        // Update state
        let mut state_lock = state.write().await;
        for symbol in symbols {
            state_lock
                .subscribed_symbols
                .insert(symbol.as_str().to_string(), true);
        }

        Ok(())
    }

    /// Sends unsubscribe message
    async fn send_unsubscribe(
        write: &mut futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            Message,
        >,
        symbols: &[Symbol],
        state: &Arc<RwLock<WsState>>,
    ) -> Result<(), BrokerError> {
        let symbol_strs: Vec<String> = symbols.iter().map(|s| s.as_str().to_string()).collect();

        let unsub_msg = serde_json::json!({
            "action": "unsubscribe",
            "quotes": symbol_strs,
            "bars": symbol_strs,
        });

        write
            .send(Message::Text(unsub_msg.to_string()))
            .await
            .map_err(|e| BrokerError::ConnectionFailed(e.to_string()))?;

        // Update state
        let mut state_lock = state.write().await;
        for symbol in symbols {
            state_lock.subscribed_symbols.remove(symbol.as_str());
        }

        Ok(())
    }
}

#[async_trait]
impl MarketDataProvider for WebSocketClient {
    async fn subscribe_quotes(&self, symbols: Vec<Symbol>) -> Result<(), BrokerError> {
        self.command_tx
            .send(WsCommand::Subscribe(symbols))
            .map_err(|e| BrokerError::ConnectionFailed(e.to_string()))
    }

    async fn unsubscribe_quotes(&self, symbols: Vec<Symbol>) -> Result<(), BrokerError> {
        self.command_tx
            .send(WsCommand::Unsubscribe(symbols))
            .map_err(|e| BrokerError::ConnectionFailed(e.to_string()))
    }

    async fn get_quote(&self, symbol: &Symbol) -> Result<Quote, BrokerError> {
        let state = self.state.read().await;
        state
            .latest_quotes
            .get(symbol.as_str())
            .cloned()
            .ok_or_else(|| BrokerError::SymbolNotFound(symbol.as_str().to_string()))
    }

    async fn get_candles(
        &self,
        _symbol: &Symbol,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
        _timeframe: Timeframe,
    ) -> Result<Vec<Candle>, BrokerError> {
        // Historical candles would be fetched via REST API, not WebSocket
        // This would typically be implemented in the main AlpacaProvider
        Err(BrokerError::ApiError(
            "Historical candles not supported via WebSocket".to_string(),
        ))
    }

    async fn start_stream(&self) -> Result<(), BrokerError> {
        let state = self.state.read().await;
        if state.is_streaming {
            Ok(())
        } else {
            Err(BrokerError::ConnectionFailed(
                "Stream not connected".to_string(),
            ))
        }
    }

    async fn stop_stream(&self) -> Result<(), BrokerError> {
        self.command_tx
            .send(WsCommand::Stop)
            .map_err(|e| BrokerError::ConnectionFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_message_serialization() {
        let auth = AuthMessage {
            action: "auth".to_string(),
            key: "test_key".to_string(),
            secret: "test_secret".to_string(),
        };
        let json = serde_json::to_string(&auth).unwrap();
        assert!(json.contains("auth"));
        assert!(json.contains("test_key"));
    }

    #[test]
    fn test_subscribe_message_serialization() {
        let sub = SubscribeMessage {
            action: "subscribe".to_string(),
            quotes: vec!["AAPL".to_string()],
            bars: vec!["AAPL".to_string()],
        };
        let json = serde_json::to_string(&sub).unwrap();
        assert!(json.contains("subscribe"));
        assert!(json.contains("AAPL"));
    }

    // Integration tests with actual WebSocket would go in tests/ directory
}
