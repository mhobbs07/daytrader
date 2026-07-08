import { useState } from "react";
import { MarketData } from "./components/MarketData";
import { PatternList } from "./components/PatternList";
import { SystemStatus } from "./components/SystemStatus";
import { RiskStatus } from "./components/RiskStatus";
import { ConnectionStatus } from "./components/ConnectionStatus";
import { StoreCleanup } from "./components/StoreCleanup";
import { Watchlist, loadWatchlist, saveWatchlist } from "./components/Watchlist";
import { TradingControls } from "./components/TradingControls";
import { OrdersPanel } from "./components/OrdersPanel";
import { PositionsPanel } from "./components/PositionsPanel";
import "./App.css";

function App() {
  const [symbols, setSymbols] = useState<string[]>(() => loadWatchlist());

  const addSymbol = (symbol: string) => {
    setSymbols((prev) => {
      const next = [...prev, symbol];
      saveWatchlist(next);
      return next;
    });
  };

  const removeSymbol = (symbol: string) => {
    setSymbols((prev) => {
      if (prev.length === 1) {
        // Always keep at least one symbol tracked.
        return prev;
      }
      const next = prev.filter((s) => s !== symbol);
      saveWatchlist(next);
      return next;
    });
  };

  return (
    <StoreCleanup>
      <main className="container">
        <header className="app-header">
          <h1>DayTrader</h1>
          <ConnectionStatus />
        </header>

        <Watchlist symbols={symbols} onAdd={addSymbol} onRemove={removeSymbol} />

        <div className="dashboard">
          <div className="dashboard-section market-data-section">
            {symbols.map((symbol) => (
              <MarketData key={symbol} symbol={symbol} />
            ))}
          </div>

          <div className="dashboard-section">
            <PatternList />
          </div>

          <div className="dashboard-section">
            <SystemStatus />
          </div>

          <div className="dashboard-section">
            <RiskStatus />
          </div>
        </div>

        <section className="trading-section">
          <div className="dashboard-section">
            <TradingControls />
          </div>

          <div className="dashboard-section trading-section-wide">
            <OrdersPanel />
          </div>

          <div className="dashboard-section trading-section-wide">
            <PositionsPanel />
          </div>
        </section>
      </main>
    </StoreCleanup>
  );
}

export default App;
