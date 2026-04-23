import { MarketData } from "./components/MarketData";
import { PatternList } from "./components/PatternList";
import { SystemStatus } from "./components/SystemStatus";
import { RiskStatus } from "./components/RiskStatus";
import { ConnectionStatus } from "./components/ConnectionStatus";
import { StoreCleanup } from "./components/StoreCleanup";
import "./App.css";

function App() {
  return (
    <StoreCleanup>
      <main className="container">
        <header className="app-header">
          <h1>DayTrader</h1>
          <ConnectionStatus />
        </header>

        <div className="dashboard">
          <div className="dashboard-section">
            <MarketData symbol="AAPL" />
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
      </main>
    </StoreCleanup>
  );
}

export default App;
