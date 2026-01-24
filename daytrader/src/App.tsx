import { MarketData } from "./components/MarketData";
import { PatternList } from "./components/PatternList";
import { SystemStatus } from "./components/SystemStatus";
import { RiskStatus } from "./components/RiskStatus";
import "./App.css";

function App() {
  return (
    <main className="container">
      <h1>DayTrader Thin Slice</h1>

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
  );
}

export default App;
