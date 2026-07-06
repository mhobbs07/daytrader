import React, { useEffect, useRef } from 'react';
import { createChart, IChartApi, ISeriesApi, UTCTimestamp } from 'lightweight-charts';
import { useMarketStore } from '../store/marketStore';

const RECENT_CANDLE_COUNT = 100;

/** Convert an ISO timestamp string into lightweight-charts' unix-seconds format. */
const toChartTime = (isoTimestamp: string): UTCTimestamp => {
  return Math.floor(new Date(isoTimestamp).getTime() / 1000) as UTCTimestamp;
};

export const CandleChart: React.FC<{ symbol: string }> = ({ symbol }) => {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const seriesRef = useRef<ISeriesApi<'Candlestick'> | null>(null);
  const lastTimeRef = useRef<UTCTimestamp | null>(null);
  const { getRecentCandles } = useMarketStore();

  // Create the chart once on mount and tear it down on unmount.
  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return;
    }

    const isDark = window.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false;
    const textColor = isDark ? '#f0f0f0' : '#0f0f0f';
    const gridColor = isDark ? '#333333' : '#e0e0e0';

    const chart = createChart(container, {
      width: container.clientWidth,
      height: container.clientHeight,
      layout: {
        background: { color: 'transparent' },
        textColor,
      },
      grid: {
        vertLines: { color: gridColor },
        horzLines: { color: gridColor },
      },
      timeScale: {
        timeVisible: true,
        secondsVisible: false,
      },
    });

    const series = chart.addCandlestickSeries({
      upColor: '#16a34a',
      downColor: '#dc2626',
      borderVisible: false,
      wickUpColor: '#16a34a',
      wickDownColor: '#dc2626',
    });

    chartRef.current = chart;
    seriesRef.current = series;

    const resizeObserver = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) {
        return;
      }
      const { width, height } = entry.contentRect;
      chart.resize(width, height);
    });
    resizeObserver.observe(container);

    return () => {
      resizeObserver.disconnect();
      chart.remove();
      chartRef.current = null;
      seriesRef.current = null;
      lastTimeRef.current = null;
    };
  }, []);

  // Backfill on mount / symbol change.
  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      try {
        const candles = await getRecentCandles(symbol, RECENT_CANDLE_COUNT);
        if (cancelled || !seriesRef.current) {
          return;
        }
        const data = candles.map((candle) => ({
          time: toChartTime(candle.timestamp),
          open: candle.open,
          high: candle.high,
          low: candle.low,
          close: candle.close,
        }));
        seriesRef.current.setData(data);
        lastTimeRef.current = data.length > 0 ? data[data.length - 1].time : null;
        chartRef.current?.timeScale().fitContent();
      } catch {
        // Backfill failures are non-fatal; live updates will still populate the chart.
      }
    };

    load();
    return () => { cancelled = true; };
  }, [symbol, getRecentCandles]);

  // Live-update from the store as new candles arrive via Tauri events.
  useEffect(() => {
    const unsubscribe = useMarketStore.subscribe((state) => {
      const symbolCandles = state.candles.get(symbol);
      if (!symbolCandles || symbolCandles.length === 0 || !seriesRef.current) {
        return;
      }
      const latest = symbolCandles[symbolCandles.length - 1];
      const time = toChartTime(latest.timestamp);
      if (lastTimeRef.current !== null && time < lastTimeRef.current) {
        return;
      }
      lastTimeRef.current = time;
      seriesRef.current.update({
        time,
        open: latest.open,
        high: latest.high,
        low: latest.low,
        close: latest.close,
      });
    });

    return unsubscribe;
  }, [symbol]);

  return <div ref={containerRef} className="candle-chart" />;
};
