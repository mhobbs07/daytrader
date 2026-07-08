import React, { useEffect } from 'react';
import { useExecutionStore } from '../store/executionStore';
import type { OrderResponse } from '../types';

function OrdersTable({ orders }: { orders: OrderResponse[] }) {
  return (
    <table className="orders-table">
      <thead>
        <tr>
          <th>Symbol</th>
          <th>Side</th>
          <th>Type</th>
          <th>Qty</th>
          <th>Limit</th>
          <th>Status</th>
          <th>Pattern</th>
          <th>Time</th>
        </tr>
      </thead>
      <tbody>
        {orders.map((order) => (
          <tr key={order.id}>
            <td className="orders-symbol">{order.symbol}</td>
            <td className={order.side.toLowerCase() === 'buy' ? 'side-buy' : 'side-sell'}>
              {order.side}
            </td>
            <td>{order.order_type}</td>
            <td>{order.quantity}</td>
            <td>{order.limit_price !== null ? `$${order.limit_price.toFixed(2)}` : '—'}</td>
            <td>
              <span className="order-status">{order.status}</span>
            </td>
            <td>{order.pattern_name ?? '—'}</td>
            <td>{new Date(order.created_at).toLocaleTimeString()}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** Active and recent order tables, sourced from the execution store. */
export const OrdersPanel: React.FC = () => {
  const { activeOrders, recentOrders, loading, error, refreshAll } = useExecutionStore();

  useEffect(() => {
    refreshAll();
  }, [refreshAll]);

  if (loading && activeOrders.length === 0 && recentOrders.length === 0) {
    return <div className="orders-panel">Loading orders...</div>;
  }

  if (error && activeOrders.length === 0 && recentOrders.length === 0) {
    return (
      <div className="orders-panel error">
        <h3>Orders</h3>
        <div className="error-message">Error: {error}</div>
        <button onClick={refreshAll}>Retry</button>
      </div>
    );
  }

  return (
    <div className="orders-panel">
      <h3>Orders</h3>

      <div className="orders-section">
        <h4>Active Orders</h4>
        {activeOrders.length === 0 ? (
          <div className="orders-empty">
            No orders yet — enable automation and add symbols to the watchlist.
          </div>
        ) : (
          <OrdersTable orders={activeOrders} />
        )}
      </div>

      <div className="orders-section">
        <h4>Recent Orders</h4>
        {recentOrders.length === 0 ? (
          <div className="orders-empty">No completed orders yet.</div>
        ) : (
          <OrdersTable orders={recentOrders} />
        )}
      </div>
    </div>
  );
};
