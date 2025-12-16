//! Day Trading Platform
//! 
//! A comprehensive pattern-based day trading system with automated execution capabilities.

#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    rust_2018_idioms,
    unreachable_pub,
    clippy::all,
    clippy::pedantic
)]

pub mod brokers;
pub mod config;
pub mod data;
pub mod db;
pub mod events;
pub mod execution;
pub mod patterns;
pub mod risk;
pub mod utils;

pub use config::Settings;
pub use utils::errors::Result;