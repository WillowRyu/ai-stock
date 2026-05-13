import { useEffect, useRef, useState } from "react";
import {
  createChart,
  type IChartApi,
  type CandlestickData,
  type LineData,
  type HistogramData,
  type Time,
} from "lightweight-charts";
import { chartIpc, type CandleInterval, type ChartDataDto, type SymbolDto } from "../lib/ipc";

// Predefined (range × interval) combinations. Combinations are chosen so the
// API returns roughly 100-500 bars per request, which both Binance (default
// limit 500) and Yahoo (limits intraday intervals to short ranges) honor.
const PRESETS = [
  { key: "1H",  days: 1,    interval: "1m"  as CandleInterval, timeVisible: true  },
  { key: "1D",  days: 1,    interval: "5m"  as CandleInterval, timeVisible: true  },
  { key: "1W",  days: 7,    interval: "30m" as CandleInterval, timeVisible: true  },
  { key: "1M",  days: 30,   interval: "1h"  as CandleInterval, timeVisible: true  },
  { key: "3M",  days: 90,   interval: "1d"  as CandleInterval, timeVisible: false },
  { key: "6M",  days: 180,  interval: "1d"  as CandleInterval, timeVisible: false },
  { key: "1Y",  days: 365,  interval: "1d"  as CandleInterval, timeVisible: false },
  { key: "5Y",  days: 1825, interval: "1w"  as CandleInterval, timeVisible: false },
] as const;
type Preset = (typeof PRESETS)[number];

const COLORS = {
  bg: "#0f172a",
  grid: "#1e293b",
  text: "#94a3b8",
  up: "#22c55e",
  down: "#ef4444",
  sma20: "#fbbf24",
  sma50: "#a78bfa",
  macd: "#60a5fa",
  signal: "#f472b6",
};

export function ChartPanel({ symbol }: { symbol: SymbolDto | null }) {
  const priceRef = useRef<HTMLDivElement>(null);
  const rsiRef = useRef<HTMLDivElement>(null);
  const macdRef = useRef<HTMLDivElement>(null);
  const chartsRef = useRef<{ price?: IChartApi; rsi?: IChartApi; macd?: IChartApi }>({});
  const [preset, setPreset] = useState<Preset>(PRESETS.find((p) => p.key === "3M")!);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  function disposeCharts() {
    chartsRef.current.price?.remove();
    chartsRef.current.rsi?.remove();
    chartsRef.current.macd?.remove();
    chartsRef.current = {};
  }

  useEffect(() => () => disposeCharts(), []);

  useEffect(() => {
    if (!symbol || !priceRef.current || !rsiRef.current || !macdRef.current) return;
    let cancelled = false;
    setLoading(true);
    setError(null);

    chartIpc
      .fetch(symbol, preset.days, preset.interval)
      .then((data) => {
        if (cancelled) return;
        renderCharts(data, preset.timeVisible);
      })
      .catch((e) => {
        if (cancelled) return;
        setError(String(e));
        disposeCharts();
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [symbol?.kind, symbol?.ticker, symbol?.quote_currency, preset.key]);

  function renderCharts(data: ChartDataDto, timeVisible: boolean) {
    disposeCharts();
    if (!priceRef.current || !rsiRef.current || !macdRef.current) return;
    if (data.candles.length === 0) {
      setError("No historical data for this symbol");
      return;
    }

    const baseOptions = {
      layout: { background: { color: COLORS.bg }, textColor: COLORS.text },
      grid: { vertLines: { color: COLORS.grid }, horzLines: { color: COLORS.grid } },
      timeScale: { borderColor: COLORS.grid, timeVisible, secondsVisible: false },
      rightPriceScale: { borderColor: COLORS.grid },
      autoSize: true,
    } as const;

    const price = createChart(priceRef.current, baseOptions);
    const candleSeries = price.addCandlestickSeries({
      upColor: COLORS.up,
      downColor: COLORS.down,
      borderVisible: false,
      wickUpColor: COLORS.up,
      wickDownColor: COLORS.down,
    });
    const sma20 = price.addLineSeries({ color: COLORS.sma20, lineWidth: 1, title: "SMA20" });
    const sma50 = price.addLineSeries({ color: COLORS.sma50, lineWidth: 1, title: "SMA50" });

    const candleData: CandlestickData<Time>[] = data.candles.map((c) => ({
      time: (Date.parse(c.opened_at) / 1000) as Time,
      open: Number(c.open),
      high: Number(c.high),
      low: Number(c.low),
      close: Number(c.close),
    }));
    candleSeries.setData(candleData);

    sma20.setData(toLineData(data.candles, data.sma_20));
    sma50.setData(toLineData(data.candles, data.sma_50));

    const rsi = createChart(rsiRef.current, baseOptions);
    const rsiSeries = rsi.addLineSeries({ color: COLORS.sma20, lineWidth: 1, title: "RSI(14)" });
    rsiSeries.setData(toLineData(data.candles, data.rsi_14));
    rsiSeries.createPriceLine({
      price: 70,
      color: "#ef4444",
      lineWidth: 1,
      lineStyle: 2,
      axisLabelVisible: true,
      title: "70",
    });
    rsiSeries.createPriceLine({
      price: 30,
      color: "#22c55e",
      lineWidth: 1,
      lineStyle: 2,
      axisLabelVisible: true,
      title: "30",
    });

    const macd = createChart(macdRef.current, baseOptions);
    const macdLine = macd.addLineSeries({ color: COLORS.macd, lineWidth: 1, title: "MACD" });
    const signalLine = macd.addLineSeries({ color: COLORS.signal, lineWidth: 1, title: "Signal" });
    const histogram = macd.addHistogramSeries({
      priceFormat: { type: "price", precision: 2, minMove: 0.01 },
    });

    macdLine.setData(toLineData(data.candles, data.macd));
    signalLine.setData(toLineData(data.candles, data.macd_signal));
    histogram.setData(toHistogramData(data.candles, data.macd_histogram));

    const syncScale = (source: IChartApi, others: IChartApi[]) => {
      source.timeScale().subscribeVisibleLogicalRangeChange((range) => {
        if (!range) return;
        others.forEach((o) => o.timeScale().setVisibleLogicalRange(range));
      });
    };
    syncScale(price, [rsi, macd]);
    syncScale(rsi, [price, macd]);
    syncScale(macd, [price, rsi]);

    chartsRef.current = { price, rsi, macd };
  }

  if (!symbol) {
    return (
      <div className="text-slate-500 text-sm p-4">차트를 보려면 워치리스트에서 종목을 선택하세요</div>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap gap-1 text-xs items-center">
        {PRESETS.map((p) => (
          <button
            key={p.key}
            onClick={() => setPreset(p)}
            title={`${p.days >= 365 ? `${Math.round(p.days / 365)}년` : p.days >= 30 ? `${Math.round(p.days / 30)}개월` : `${p.days}일`} · ${p.interval}봉`}
            className={
              "px-2 py-1 rounded " +
              (preset.key === p.key ? "bg-emerald-600" : "bg-slate-800 hover:bg-slate-700")
            }
          >
            {p.key}
          </button>
        ))}
        <span className="text-[10px] text-slate-500 ml-2">{preset.interval} 봉</span>
        {loading && <span className="text-slate-500 ml-2">로딩...</span>}
      </div>
      {error && <div className="text-rose-400 text-xs">{error}</div>}
      <div className="text-[10px] text-slate-500">가격 · SMA20(노랑) · SMA50(보라)</div>
      <div ref={priceRef} className="h-72 bg-slate-950 rounded border border-slate-800" />
      <div className="text-[10px] text-slate-500">RSI(14) · 30/70 기준선</div>
      <div ref={rsiRef} className="h-24 bg-slate-950 rounded border border-slate-800" />
      <div className="text-[10px] text-slate-500">MACD(12,26,9) · 시그널/히스토그램</div>
      <div ref={macdRef} className="h-24 bg-slate-950 rounded border border-slate-800" />
    </div>
  );
}

function toLineData(
  candles: { opened_at: string }[],
  series: (string | null)[],
): LineData<Time>[] {
  const out: LineData<Time>[] = [];
  for (let i = 0; i < candles.length; i++) {
    const v = series[i];
    if (v === null || v === undefined) continue;
    out.push({
      time: (Date.parse(candles[i].opened_at) / 1000) as Time,
      value: Number(v),
    });
  }
  return out;
}

function toHistogramData(
  candles: { opened_at: string }[],
  series: (string | null)[],
): HistogramData<Time>[] {
  const out: HistogramData<Time>[] = [];
  for (let i = 0; i < candles.length; i++) {
    const v = series[i];
    if (v === null || v === undefined) continue;
    const n = Number(v);
    out.push({
      time: (Date.parse(candles[i].opened_at) / 1000) as Time,
      value: n,
      color: n >= 0 ? COLORS.up : COLORS.down,
    });
  }
  return out;
}
