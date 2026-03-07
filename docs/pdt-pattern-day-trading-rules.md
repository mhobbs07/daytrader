# Pattern Day Trading (PDT) Rules and Regulations

## Overview

Pattern Day Trading (PDT) rules are SEC and FINRA regulations designed to protect retail investors from the risks associated with frequent day trading in margin accounts. These rules apply to all US brokerage accounts and establish minimum equity requirements and trade limitations.

## Key Definitions

### Day Trade
A **day trade** is defined as:
- Buying and selling (or selling and buying) the same security on the same trading day
- In a margin account
- The trades must be in the same underlying security (including options on the same security)

### Pattern Day Trader
A **Pattern Day Trader (PDT)** is defined as:
- Any customer who executes **4 or more day trades** within any **5 consecutive business days**
- The day trades must represent more than 6% of total trading activity during the same period
- This designation applies to margin accounts only

## Core PDT Requirements

### 1. Minimum Equity Requirement
- **$25,000 minimum equity**: Pattern Day Traders must maintain at least $25,000 in account equity at all times
- Equity includes: cash, marginable securities, and non-marginable securities at market value
- This requirement must be met **before** any day trading activity can occur on a given day

### 2. Day Trading Buying Power
- Pattern Day Traders receive **4:1 intraday leverage** (compared to 2:1 overnight)
- Day trading buying power = (Previous day's closing equity - Previous day's maintenance margin requirement) × 4
- This buying power resets at the beginning of each trading day

### 3. Account Restrictions for Under-$25k Accounts
- Accounts with less than $25,000 equity are limited to **3 day trades in any 5 consecutive business days**
- After 3 day trades, the account is restricted from further day trading until:
  - The account equity exceeds $25,000, OR
  - The 5-day period resets

## Business Day Calculation

### What Counts as a Business Day
- Monday through Friday
- Excludes weekends (Saturday and Sunday)
- Excludes major market holidays:
  - New Year's Day
  - Martin Luther King Jr. Day
  - Presidents' Day
  - Good Friday
  - Memorial Day
  - Independence Day
  - Labor Day
  - Thanksgiving Day
  - Christmas Day

### Rolling 5-Day Window
The PDT rule uses a **rolling 5 business day window**, meaning:
- Each day, the calculation looks back 5 business days
- Old trades automatically "fall off" the count after 5 business days
- The count is recalculated continuously

## Trade Counting Examples

### Example 1: Day Trade Identification
```
9:30 AM: Buy 100 shares AAPL at $150.00
2:00 PM: Sell 100 shares AAPL at $152.00
```
This is **1 day trade** (buy then sell same day).

### Example 2: Not a Day Trade
```
Monday: Buy 100 shares AAPL
Tuesday: Sell 100 shares AAPL
```
This is **not a day trade** (positions held overnight).

### Example 3: Multiple Day Trades Same Stock
```
9:30 AM: Buy 200 shares AAPL
11:00 AM: Sell 100 shares AAPL (1st day trade)
1:00 PM: Sell remaining 100 shares AAPL (2nd day trade)
```
This counts as **2 separate day trades**.

## Account Types and PDT Rules

### Margin Accounts
- PDT rules apply to all margin accounts
- Subject to minimum equity requirements
- Receive enhanced day trading buying power when above $25k

### Cash Accounts
- **PDT rules do NOT apply** to cash accounts
- No minimum equity requirements for day trading
- Limited by cash settlement rules (T+2 for stocks)
- Cannot use unsettled funds for day trading

## Enforcement and Violations

### PDT Call (Minimum Equity Call)
If account equity falls below $25,000:
- Account is restricted from day trading until equity requirement is met
- Must deposit funds to bring account above $25,000
- Restriction remains until deposit clears and equity is verified

### Day Trading Call
If day trading buying power is exceeded:
- Account has 5 business days to meet the call
- Day trading is restricted until call is met
- Failure to meet call results in account restrictions

### Account Restrictions
When PDT violations occur:
- **90-day restriction** on day trading activity
- Account can only close positions, not open new day trades
- Restriction can only be removed by meeting equity requirements

## Regulatory Authority and References

### Primary Regulators
- **Securities and Exchange Commission (SEC)**: Federal oversight
- **Financial Industry Regulatory Authority (FINRA)**: Self-regulatory organization
- **Individual Brokers**: Implement and enforce rules

### Key Regulatory Documents
- **FINRA Rule 4210**: Day-Trading Margin Requirements
- **SEC Regulation T**: Credit by Brokers and Dealers
- **FINRA Regulatory Notice 01-27**: Day Trading Margin Requirements

### Official Sources
- [FINRA Day Trading Rules](https://www.finra.org/investors/learn-to-invest/advanced-investing/day-trading-margin-requirements-know-rules)
- [SEC Day Trading Information](https://www.sec.gov/investor/pubs/daytips.htm)
- [FINRA Rule 4210](https://www.finra.org/rules-guidance/rulebooks/finra-rules/4210)

## Compliance Best Practices

### For Trading Platforms
1. **Real-time Monitoring**: Track day trades in real-time across the 5-day window
2. **Pre-trade Validation**: Check PDT status before allowing day trades
3. **Automated Restrictions**: Implement automatic restrictions when limits are reached
4. **Clear Notifications**: Alert users when approaching PDT limits
5. **Accurate Counting**: Ensure proper identification of day trades vs. overnight positions

### For Traders
1. **Account Monitoring**: Regularly check account equity and day trade count
2. **Position Management**: Plan entries and exits to minimize day trades
3. **Cash Account Option**: Consider cash accounts if day trading with less than $25k
4. **Risk Management**: Understand that PDT rules exist to limit risk exposure

## Common Misconceptions

### Myth: PDT Rules Apply to All Account Types
**Reality**: PDT rules only apply to margin accounts. Cash accounts are exempt but subject to cash settlement rules.

### Myth: Weekend Days Count in 5-Day Window
**Reality**: Only business days (Monday-Friday, excluding holidays) count in the rolling window.

### Myth: Different Brokers Have Different Rules
**Reality**: PDT rules are federal regulations that apply uniformly across all US brokers.

### Myth: Options Don't Count as Day Trades
**Reality**: Options on the same underlying security count as day trades if bought and sold on the same day.

## Implementation Considerations

### Technical Requirements
- Real-time position tracking
- Business day calculation logic
- Rolling window maintenance
- Account equity monitoring
- Automated restriction enforcement

### Data Storage
- Trade history with timestamps
- Account equity snapshots
- PDT status tracking
- Violation history

### User Interface
- Clear PDT status display
- Day trade count indicators
- Warning notifications
- Educational resources

## Updates and Changes

This document reflects PDT rules as of January 2024. Regulations may change over time. Always consult current FINRA and SEC resources for the most up-to-date information.

**Last Updated**: January 24, 2024  
**Document Version**: 1.0  
**Regulatory Review**: Required annually