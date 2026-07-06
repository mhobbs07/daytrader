//! US Market Holiday Calendar
//!
//! This module provides a comprehensive holiday calendar for US stock markets (NYSE/NASDAQ)
//! with accurate holiday calculations, observation rules, and business day determination.
//!
//! # Features
//!
//! - **Complete US Market Holidays**: All major trading holidays including floating holidays
//! - **Observation Rules**: Proper handling when holidays fall on weekends
//! - **Business Day Calculator**: Fast business day determination with caching
//! - **Thread-Safe**: All operations are thread-safe for concurrent access
//! - **Performance Optimized**: Holiday calculations cached for sub-100μs lookups
//!
//! # Examples
//!
//! ```
//! use daytrader::utils::holidays::{MarketHolidayCalendar, is_business_day};
//! use chrono::NaiveDate;
//!
//! // Check if a date is a business day
//! let date = NaiveDate::from_ymd_opt(2024, 12, 25).unwrap(); // Christmas
//! assert!(!is_business_day(date));
//!
//! // Get holiday calendar
//! let calendar = MarketHolidayCalendar::new();
//! let holidays = calendar.get_holidays_for_year(2024);
//! ```

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use thiserror::Error;

/// Errors that can occur during holiday calendar operations
#[derive(Error, Debug)]
pub enum HolidayError {
    #[error("Invalid year: {0}. Year must be between 1900 and 2100")]
    InvalidYear(i32),
    #[error("Failed to calculate Easter for year {0}")]
    EasterCalculationError(i32),
    #[error("Invalid date: {0}")]
    InvalidDate(String),
}

/// Configuration for holiday calendar customization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolidayConfig {
    /// Enable/disable specific holidays
    pub enabled_holidays: HashSet<String>,
    /// Custom additional holidays (name, month, day)
    pub custom_holidays: Vec<(String, u32, u32)>,
    /// Whether to cache holiday calculations
    pub enable_cache: bool,
    /// Maximum years to cache ahead
    pub cache_years_ahead: u32,
}

impl Default for HolidayConfig {
    fn default() -> Self {
        let mut enabled_holidays = HashSet::new();
        enabled_holidays.insert("new_years_day".to_string());
        enabled_holidays.insert("martin_luther_king_day".to_string());
        enabled_holidays.insert("presidents_day".to_string());
        enabled_holidays.insert("good_friday".to_string());
        enabled_holidays.insert("memorial_day".to_string());
        enabled_holidays.insert("independence_day".to_string());
        enabled_holidays.insert("labor_day".to_string());
        enabled_holidays.insert("thanksgiving_day".to_string());
        enabled_holidays.insert("christmas_day".to_string());

        Self {
            enabled_holidays,
            custom_holidays: Vec::new(),
            enable_cache: true,
            cache_years_ahead: 5,
        }
    }
}

/// Represents a market holiday with its calculation rule
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holiday {
    pub name: String,
    pub date: NaiveDate,
    pub observed_date: NaiveDate,
}

impl Holiday {
    pub fn new(name: String, date: NaiveDate) -> Self {
        let observed_date = apply_observation_rules(date);
        Self {
            name,
            date,
            observed_date,
        }
    }
}

/// US Market Holiday Calendar with caching and thread-safe access
#[derive(Debug, Clone)]
pub struct MarketHolidayCalendar {
    config: HolidayConfig,
    /// Cache of holidays by year
    holiday_cache: Arc<RwLock<HashMap<i32, Vec<Holiday>>>>,
    /// Fast lookup set for business day checks
    business_day_cache: Arc<RwLock<HashMap<NaiveDate, bool>>>,
}

impl MarketHolidayCalendar {
    /// Creates a new market holiday calendar with default configuration
    pub fn new() -> Self {
        Self::with_config(HolidayConfig::default())
    }

    /// Creates a new market holiday calendar with custom configuration
    pub fn with_config(config: HolidayConfig) -> Self {
        let calendar = Self {
            config,
            holiday_cache: Arc::new(RwLock::new(HashMap::new())),
            business_day_cache: Arc::new(RwLock::new(HashMap::new())),
        };

        // Pre-populate cache for current and future years if enabled
        if calendar.config.enable_cache {
            let current_year = chrono::Utc::now().year();
            for year in current_year..(current_year + calendar.config.cache_years_ahead as i32 + 1)
            {
                if let Ok(holidays) = calendar.calculate_holidays_for_year(year) {
                    calendar
                        .holiday_cache
                        .write()
                        .unwrap()
                        .insert(year, holidays);
                }
            }
        }

        calendar
    }

    /// Gets all holidays for a specific year
    pub fn get_holidays_for_year(&self, year: i32) -> Result<Vec<Holiday>, HolidayError> {
        if !(1900..=2100).contains(&year) {
            return Err(HolidayError::InvalidYear(year));
        }

        // Check cache first
        if self.config.enable_cache {
            if let Ok(cache) = self.holiday_cache.read() {
                if let Some(holidays) = cache.get(&year) {
                    return Ok(holidays.clone());
                }
            }
        }

        // Calculate and cache if enabled
        let holidays = self.calculate_holidays_for_year(year)?;

        if self.config.enable_cache {
            if let Ok(mut cache) = self.holiday_cache.write() {
                cache.insert(year, holidays.clone());
            }
        }

        Ok(holidays)
    }

    /// Calculates holidays for a specific year
    fn calculate_holidays_for_year(&self, year: i32) -> Result<Vec<Holiday>, HolidayError> {
        let mut holidays = Vec::new();

        // New Year's Day (January 1)
        if self.config.enabled_holidays.contains("new_years_day") {
            let date = NaiveDate::from_ymd_opt(year, 1, 1)
                .ok_or_else(|| HolidayError::InvalidDate(format!("Jan 1, {}", year)))?;
            holidays.push(Holiday::new("New Year's Day".to_string(), date));
        }

        // Martin Luther King Jr. Day (3rd Monday in January)
        if self
            .config
            .enabled_holidays
            .contains("martin_luther_king_day")
        {
            let date = third_monday_of_month(year, 1)?;
            holidays.push(Holiday::new("Martin Luther King Jr. Day".to_string(), date));
        }

        // Presidents' Day / Washington's Birthday (3rd Monday in February)
        if self.config.enabled_holidays.contains("presidents_day") {
            let date = third_monday_of_month(year, 2)?;
            holidays.push(Holiday::new("Presidents' Day".to_string(), date));
        }

        // Good Friday (Friday before Easter Sunday)
        if self.config.enabled_holidays.contains("good_friday") {
            let easter = calculate_easter(year)?;
            let good_friday = easter - Duration::days(2);
            holidays.push(Holiday::new("Good Friday".to_string(), good_friday));
        }

        // Memorial Day (Last Monday in May)
        if self.config.enabled_holidays.contains("memorial_day") {
            let date = last_monday_of_month(year, 5)?;
            holidays.push(Holiday::new("Memorial Day".to_string(), date));
        }

        // Independence Day (July 4)
        if self.config.enabled_holidays.contains("independence_day") {
            let date = NaiveDate::from_ymd_opt(year, 7, 4)
                .ok_or_else(|| HolidayError::InvalidDate(format!("July 4, {}", year)))?;
            holidays.push(Holiday::new("Independence Day".to_string(), date));
        }

        // Labor Day (1st Monday in September)
        if self.config.enabled_holidays.contains("labor_day") {
            let date = first_monday_of_month(year, 9)?;
            holidays.push(Holiday::new("Labor Day".to_string(), date));
        }

        // Thanksgiving Day (4th Thursday in November)
        if self.config.enabled_holidays.contains("thanksgiving_day") {
            let date = fourth_thursday_of_month(year, 11)?;
            holidays.push(Holiday::new("Thanksgiving Day".to_string(), date));
        }

        // Christmas Day (December 25)
        if self.config.enabled_holidays.contains("christmas_day") {
            let date = NaiveDate::from_ymd_opt(year, 12, 25)
                .ok_or_else(|| HolidayError::InvalidDate(format!("Dec 25, {}", year)))?;
            holidays.push(Holiday::new("Christmas Day".to_string(), date));
        }

        // Add custom holidays
        for (name, month, day) in &self.config.custom_holidays {
            if let Some(date) = NaiveDate::from_ymd_opt(year, *month, *day) {
                holidays.push(Holiday::new(name.clone(), date));
            }
        }

        // Sort by date
        holidays.sort_by_key(|h| h.observed_date);

        Ok(holidays)
    }

    /// Checks if a date is a holiday
    pub fn is_holiday(&self, date: NaiveDate) -> bool {
        let year = date.year();
        if let Ok(holidays) = self.get_holidays_for_year(year) {
            holidays.iter().any(|h| h.observed_date == date)
        } else {
            false
        }
    }

    /// Checks if a date is a business day (not weekend and not holiday)
    pub fn is_business_day(&self, date: NaiveDate) -> bool {
        // Check cache first
        if self.config.enable_cache {
            if let Ok(cache) = self.business_day_cache.read() {
                if let Some(&is_business_day) = cache.get(&date) {
                    return is_business_day;
                }
            }
        }

        // Calculate business day status
        let is_business_day = !is_weekend(date) && !self.is_holiday(date);

        // Cache the result if caching is enabled
        if self.config.enable_cache {
            if let Ok(mut cache) = self.business_day_cache.write() {
                cache.insert(date, is_business_day);
            }
        }

        is_business_day
    }

    /// Counts business days between two dates (exclusive of end date)
    pub fn business_days_between(&self, start: NaiveDate, end: NaiveDate) -> i64 {
        let mut count = 0;
        let mut current = start;

        while current < end {
            if self.is_business_day(current) {
                count += 1;
            }
            current += Duration::days(1);
        }

        count
    }

    /// Adds business days to a date, returning the resulting date
    pub fn add_business_days(&self, date: NaiveDate, business_days: i32) -> NaiveDate {
        let mut current = date;
        let mut days_added = 0;
        let increment = if business_days >= 0 { 1 } else { -1 };
        let target_days = business_days.abs();

        while days_added < target_days {
            current += Duration::days(increment as i64);
            if self.is_business_day(current) {
                days_added += 1;
            }
        }

        current
    }

    /// Subtracts business days from a date, returning the resulting date
    pub fn subtract_business_days(&self, date: NaiveDate, business_days: i32) -> NaiveDate {
        self.add_business_days(date, -business_days)
    }

    /// Clears the holiday cache (useful for testing or configuration changes)
    pub fn clear_cache(&self) {
        if let Ok(mut holiday_cache) = self.holiday_cache.write() {
            holiday_cache.clear();
        }
        if let Ok(mut business_cache) = self.business_day_cache.write() {
            business_cache.clear();
        }
    }

    /// Gets cache statistics for monitoring
    pub fn cache_stats(&self) -> (usize, usize) {
        let holiday_count = self.holiday_cache.read().map(|c| c.len()).unwrap_or(0);
        let business_count = self.business_day_cache.read().map(|c| c.len()).unwrap_or(0);
        (holiday_count, business_count)
    }
}

impl Default for MarketHolidayCalendar {
    fn default() -> Self {
        Self::new()
    }
}

/// Global holiday calendar instance for convenient access
static GLOBAL_CALENDAR: std::sync::OnceLock<MarketHolidayCalendar> = std::sync::OnceLock::new();

/// Gets the global holiday calendar instance
pub fn global_calendar() -> &'static MarketHolidayCalendar {
    GLOBAL_CALENDAR.get_or_init(|| MarketHolidayCalendar::new())
}

/// Convenient function to check if a date is a business day
pub fn is_business_day(date: NaiveDate) -> bool {
    global_calendar().is_business_day(date)
}

/// Convenient function to check if a date is a holiday
pub fn is_holiday(date: NaiveDate) -> bool {
    global_calendar().is_holiday(date)
}

/// Convenient function to check if a date is a weekend
pub fn is_weekend(date: NaiveDate) -> bool {
    matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
}

/// Apply US market holiday observation rules
/// - If holiday falls on Saturday, observed on Friday
/// - If holiday falls on Sunday, observed on Monday
fn apply_observation_rules(date: NaiveDate) -> NaiveDate {
    match date.weekday() {
        Weekday::Sat => date - Duration::days(1), // Friday
        Weekday::Sun => date + Duration::days(1), // Monday
        _ => date,                                // Weekday - no change
    }
}

/// Calculates Easter Sunday for a given year using the Gregorian calendar
/// Uses the algorithm by Jean Meeus
fn calculate_easter(year: i32) -> Result<NaiveDate, HolidayError> {
    // Algorithm for Gregorian calendar (valid for years 1583-4099)
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = ((h + l - 7 * m + 114) % 31) + 1;

    NaiveDate::from_ymd_opt(year, month as u32, day as u32)
        .ok_or_else(|| HolidayError::EasterCalculationError(year))
}

/// Returns the first Monday of a given month and year
fn first_monday_of_month(year: i32, month: u32) -> Result<NaiveDate, HolidayError> {
    let first_day = NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(|| {
        HolidayError::InvalidDate(format!("First day of month {}/{}", month, year))
    })?;

    // Find the first Monday
    let days_to_monday = match first_day.weekday() {
        Weekday::Mon => 0,
        Weekday::Tue => 6,
        Weekday::Wed => 5,
        Weekday::Thu => 4,
        Weekday::Fri => 3,
        Weekday::Sat => 2,
        Weekday::Sun => 1,
    };

    Ok(first_day + Duration::days(days_to_monday))
}

/// Returns the third Monday of a given month and year
fn third_monday_of_month(year: i32, month: u32) -> Result<NaiveDate, HolidayError> {
    let first_monday = first_monday_of_month(year, month)?;
    Ok(first_monday + Duration::days(14)) // Add 2 weeks
}

/// Returns the fourth Thursday of a given month and year
fn fourth_thursday_of_month(year: i32, month: u32) -> Result<NaiveDate, HolidayError> {
    let first_day = NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(|| {
        HolidayError::InvalidDate(format!("First day of month {}/{}", month, year))
    })?;

    // Find the first Thursday
    let days_to_thursday = match first_day.weekday() {
        Weekday::Mon => 3,
        Weekday::Tue => 2,
        Weekday::Wed => 1,
        Weekday::Thu => 0,
        Weekday::Fri => 6,
        Weekday::Sat => 5,
        Weekday::Sun => 4,
    };

    let first_thursday = first_day + Duration::days(days_to_thursday);
    Ok(first_thursday + Duration::days(21)) // Add 3 weeks for fourth Thursday
}

/// Returns the last Monday of a given month and year
fn last_monday_of_month(year: i32, month: u32) -> Result<NaiveDate, HolidayError> {
    // Get the last day of the month
    let next_month_first = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };

    let last_day = next_month_first
        .ok_or_else(|| HolidayError::InvalidDate(format!("Next month after {}/{}", month, year)))?
        - Duration::days(1);

    // Find the last Monday
    let days_back_to_monday = match last_day.weekday() {
        Weekday::Mon => 0,
        Weekday::Tue => 1,
        Weekday::Wed => 2,
        Weekday::Thu => 3,
        Weekday::Fri => 4,
        Weekday::Sat => 5,
        Weekday::Sun => 6,
    };

    Ok(last_day - Duration::days(days_back_to_monday))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_holiday_observation_rules() {
        // Christmas 2021 fell on Saturday, should be observed on Friday (Dec 24)
        let christmas_2021 = NaiveDate::from_ymd_opt(2021, 12, 25).unwrap();
        let observed = apply_observation_rules(christmas_2021);
        assert_eq!(observed, NaiveDate::from_ymd_opt(2021, 12, 24).unwrap());

        // New Year's 2023 fell on Sunday, should be observed on Monday (Jan 2)
        let new_years_2023 = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
        let observed = apply_observation_rules(new_years_2023);
        assert_eq!(observed, NaiveDate::from_ymd_opt(2023, 1, 2).unwrap());

        // Regular weekday should not change
        let weekday = NaiveDate::from_ymd_opt(2023, 7, 4).unwrap(); // Tuesday
        let observed = apply_observation_rules(weekday);
        assert_eq!(observed, weekday);
    }

    #[test]
    fn test_easter_calculation() {
        // Test known Easter dates
        assert_eq!(
            calculate_easter(2023).unwrap(),
            NaiveDate::from_ymd_opt(2023, 4, 9).unwrap()
        );
        assert_eq!(
            calculate_easter(2024).unwrap(),
            NaiveDate::from_ymd_opt(2024, 3, 31).unwrap()
        );
        assert_eq!(
            calculate_easter(2025).unwrap(),
            NaiveDate::from_ymd_opt(2025, 4, 20).unwrap()
        );
    }

    #[test]
    fn test_floating_holidays() {
        // Martin Luther King Jr. Day 2023 (3rd Monday in January)
        let mlk_2023 = third_monday_of_month(2023, 1).unwrap();
        assert_eq!(mlk_2023, NaiveDate::from_ymd_opt(2023, 1, 16).unwrap());

        // Memorial Day 2023 (Last Monday in May)
        let memorial_2023 = last_monday_of_month(2023, 5).unwrap();
        assert_eq!(memorial_2023, NaiveDate::from_ymd_opt(2023, 5, 29).unwrap());

        // Labor Day 2023 (1st Monday in September)
        let labor_2023 = first_monday_of_month(2023, 9).unwrap();
        assert_eq!(labor_2023, NaiveDate::from_ymd_opt(2023, 9, 4).unwrap());

        // Thanksgiving 2023 (4th Thursday in November)
        let thanksgiving_2023 = fourth_thursday_of_month(2023, 11).unwrap();
        assert_eq!(
            thanksgiving_2023,
            NaiveDate::from_ymd_opt(2023, 11, 23).unwrap()
        );
    }

    #[test]
    fn test_market_holiday_calendar() {
        let calendar = MarketHolidayCalendar::new();

        // Test 2023 holidays
        let holidays_2023 = calendar.get_holidays_for_year(2023).unwrap();
        assert_eq!(holidays_2023.len(), 9); // All major market holidays

        // Check specific holidays
        let holiday_dates: Vec<NaiveDate> = holidays_2023.iter().map(|h| h.observed_date).collect();

        // New Year's Day 2023 (observed on Monday Jan 2 because Jan 1 was Sunday)
        assert!(holiday_dates.contains(&NaiveDate::from_ymd_opt(2023, 1, 2).unwrap()));

        // Christmas Day 2023 (Monday, no observation change)
        assert!(holiday_dates.contains(&NaiveDate::from_ymd_opt(2023, 12, 25).unwrap()));
    }

    #[test]
    fn test_business_day_checking() {
        let calendar = MarketHolidayCalendar::new();

        // Test weekend
        let saturday = NaiveDate::from_ymd_opt(2023, 7, 1).unwrap();
        assert!(!calendar.is_business_day(saturday));

        // Test holiday
        let christmas = NaiveDate::from_ymd_opt(2023, 12, 25).unwrap();
        assert!(!calendar.is_business_day(christmas));

        // Test regular business day
        let regular_day = NaiveDate::from_ymd_opt(2023, 7, 5).unwrap(); // Wednesday
        assert!(calendar.is_business_day(regular_day));
    }

    #[test]
    fn test_business_days_calculation() {
        let calendar = MarketHolidayCalendar::new();

        // Test adding business days
        let start_date = NaiveDate::from_ymd_opt(2023, 12, 21).unwrap(); // Thursday
        let result = calendar.add_business_days(start_date, 1);
        assert_eq!(result, NaiveDate::from_ymd_opt(2023, 12, 22).unwrap()); // Friday

        // Test adding business days across weekend and holiday
        let result = calendar.add_business_days(start_date, 2);
        assert_eq!(result, NaiveDate::from_ymd_opt(2023, 12, 26).unwrap()); // Tuesday (skips weekend + Christmas)
    }

    #[test]
    fn test_global_calendar_functions() {
        // Test weekend
        assert!(!is_business_day(
            NaiveDate::from_ymd_opt(2023, 7, 1).unwrap()
        )); // Saturday

        // Test holiday
        assert!(is_holiday(NaiveDate::from_ymd_opt(2023, 12, 25).unwrap())); // Christmas

        // Test weekend function
        assert!(is_weekend(NaiveDate::from_ymd_opt(2023, 7, 2).unwrap())); // Sunday
    }

    #[test]
    fn test_cache_functionality() {
        let calendar = MarketHolidayCalendar::new();

        // Check cache stats after initialization
        let (holiday_count, _) = calendar.cache_stats();
        assert!(holiday_count > 0); // Should have cached several years

        // Clear cache and verify
        calendar.clear_cache();
        let (holiday_count_after, business_count_after) = calendar.cache_stats();
        assert_eq!(holiday_count_after, 0);
        assert_eq!(business_count_after, 0);
    }

    #[test]
    fn test_performance_characteristics() {
        let calendar = MarketHolidayCalendar::new();
        let test_date = NaiveDate::from_ymd_opt(2023, 7, 4).unwrap();

        // First call may be slower due to cache miss
        let start = std::time::Instant::now();
        let is_business = calendar.is_business_day(test_date);
        let first_call_duration = start.elapsed();

        // Second call should be much faster due to cache hit
        let start = std::time::Instant::now();
        let is_business_cached = calendar.is_business_day(test_date);
        let second_call_duration = start.elapsed();

        assert_eq!(is_business, is_business_cached);
        assert!(second_call_duration < first_call_duration);
        // Should be well under 100μs for cached lookups
        assert!(second_call_duration.as_micros() < 100);
    }
}
