import { useEffect, useRef, useState } from "react";
import {
  createChart,
  type IChartApi,
  type CandlestickData,
  type LineData,
  type HistogramData,
  type Time,
  type PriceFormat,
} from "lightweight-charts";
import { chartIpc, type CandleInterval, type ChartDataDto, type SymbolDto } from "../lib/ipc";
import { formatPrice } from "../lib/format";

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

interface IndicatorVisibility {
  sma20: boolean;
  sma50: boolean;
  rsi: boolean;
  macd: boolean;
}

export function ChartPanel({ symbol }: { symbol: SymbolDto | null }) {
  const priceRef = useRef<HTMLDivElement>(null);
  const rsiRef = useRef<HTMLDivElement>(null);
  const macdRef = useRef<HTMLDivElement>(null);
  const volumeRef = useRef<HTMLDivElement>(null);
  const chartsRef = useRef<{
    price?: IChartApi;
    rsi?: IChartApi;
    macd?: IChartApi;
    volume?: IChartApi;
  }>({});
  const [preset, setPreset] = useState<Preset>(PRESETS.find((p) => p.key === "3M")!);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [show, setShow] = useState<IndicatorVisibility>({
    sma20: true,
    sma50: true,
    rsi: true,
    macd: true,
  });

  function disposeCharts() {
    chartsRef.current.price?.remove();
    chartsRef.current.rsi?.remove();
    chartsRef.current.macd?.remove();
    chartsRef.current.volume?.remove();
    chartsRef.current = {};
  }

  useEffect(() => () => disposeCharts(), []);

  useEffect(() => {
    if (!symbol || !priceRef.current) return;
    let cancelled = false;
    setLoading(true);
    setError(null);

    chartIpc
      .fetch(symbol, preset.days, preset.interval)
      .then((data) => {
        if (cancelled) return;
        renderCharts(data, preset.timeVisible, show);
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
  }, [
    symbol?.kind,
    symbol?.ticker,
    symbol?.quote_currency,
    preset.key,
    show.sma20,
    show.sma50,
    show.rsi,
    show.macd,
  ]);

  function renderCharts(data: ChartDataDto, timeVisible: boolean, vis: IndicatorVisibility) {
    disposeCharts();
    if (!priceRef.current) return;
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

    const priceFormat: PriceFormat = {
      type: "custom",
      minMove: 0.0001,
      formatter: (p: number) => formatPrice(String(p)),
    };

    const price = createChart(priceRef.current, baseOptions);
    const candleSeries = price.addCandlestickSeries({
      upColor: COLORS.up,
      downColor: COLORS.down,
      borderVisible: false,
      wickUpColor: COLORS.up,
      wickDownColor: COLORS.down,
    });
    candleSeries.applyOptions({ priceFormat });

    const candleData: CandlestickData<Time>[] = data.candles.map((c) => ({
      time: (Date.parse(c.opened_at) / 1000) as Time,
      open: Number(c.open),
      high: Number(c.high),
      low: Number(c.low),
      close: Number(c.close),
    }));
    candleSeries.setData(candleData);

    if (vis.sma20) {
      const sma20 = price.addLineSeries({ color: COLORS.sma20, lineWidth: 1, title: "SMA20" });
      sma20.applyOptions({ priceFormat });
      sma20.setData(toLineData(data.candles, data.sma_20));
    }
    if (vis.sma50) {
      const sma50 = price.addLineSeries({ color: COLORS.sma50, lineWidth: 1, title: "SMA50" });
      sma50.applyOptions({ priceFormat });
      sma50.setData(toLineData(data.candles, data.sma_50));
    }

    chartsRef.current.price = price;
    const synced: IChartApi[] = [price];

    if (vis.rsi && rsiRef.current) {
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
      chartsRef.current.rsi = rsi;
      synced.push(rsi);
    }

    if (vis.macd && macdRef.current) {
      const macd = createChart(macdRef.current, baseOptions);
      const macdLine = macd.addLineSeries({ color: COLORS.macd, lineWidth: 1, title: "MACD" });
      const signalLine = macd.addLineSeries({ color: COLORS.signal, lineWidth: 1, title: "Signal" });
      const histogram = macd.addHistogramSeries({
        priceFormat: { type: "price", precision: 2, minMove: 0.01 },
      });
      macdLine.setData(toLineData(data.candles, data.macd));
      signalLine.setData(toLineData(data.candles, data.macd_signal));
      histogram.setData(toHistogramData(data.candles, data.macd_histogram));
      chartsRef.current.macd = macd;
      synced.push(macd);
    }

    if (volumeRef.current) {
      const volume = createChart(volumeRef.current, baseOptions);
      const volSeries = volume.addHistogramSeries({
        priceFormat: { type: "volume" },
      });
      volSeries.setData(toVolumeData(data.candles));
      chartsRef.current.volume = volume;
      synced.push(volume);
    }

    const syncScale = (source: IChartApi, others: IChartApi[]) => {
      source.timeScale().subscribeVisibleLogicalRangeChange((range) => {
        if (!range) return;
        others.forEach((o) => o.timeScale().setVisibleLogicalRange(range));
      });
    };
    for (const ch of synced) {
      syncScale(ch, synced.filter((o) => o !== ch));
    }
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
      <div className="flex flex-wrap gap-3 text-xs items-center text-slate-300">
        <IndicatorToggle
          label="SMA20"
          checked={show.sma20}
          onChange={(v) => setShow((s) => ({ ...s, sma20: v }))}
        />
        <IndicatorToggle
          label="SMA50"
          checked={show.sma50}
          onChange={(v) => setShow((s) => ({ ...s, sma50: v }))}
        />
        <IndicatorToggle
          label="RSI"
          checked={show.rsi}
          onChange={(v) => setShow((s) => ({ ...s, rsi: v }))}
        />
        <IndicatorToggle
          label="MACD"
          checked={show.macd}
          onChange={(v) => setShow((s) => ({ ...s, macd: v }))}
        />
      </div>
      {error && <div className="text-rose-400 text-xs">{error}</div>}
      <div className="text-[10px] text-slate-500">
        가격
        {show.sma20 && " · SMA20(노랑)"}
        {show.sma50 && " · SMA50(보라)"}
      </div>
      <div ref={priceRef} className="h-72 bg-slate-950 rounded border border-slate-800" />
      {show.rsi && (
        <>
          <div className="text-[10px] text-slate-500">RSI(14) · 30/70 기준선</div>
          <div ref={rsiRef} className="h-24 bg-slate-950 rounded border border-slate-800" />
        </>
      )}
      {show.macd && (
        <>
          <div className="text-[10px] text-slate-500">MACD(12,26,9) · 시그널/히스토그램</div>
          <div ref={macdRef} className="h-24 bg-slate-950 rounded border border-slate-800" />
        </>
      )}
      <div className="text-[10px] text-slate-500">거래량</div>
      <div ref={volumeRef} className="h-16 bg-slate-950 rounded border border-slate-800" />
    </div>
  );
}

function IndicatorToggle({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange(v: boolean): void;
}) {
  return (
    <label className="inline-flex items-center gap-1 cursor-pointer select-none">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="accent-emerald-500"
      />
      <span>{label}</span>
    </label>
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

function toVolumeData(
  candles: { opened_at: string; open: string; close: string; volume: string }[],
): HistogramData<Time>[] {
  const out: HistogramData<Time>[] = [];
  for (const c of candles) {
    const v = Number(c.volume);
    if (!Number.isFinite(v)) continue;
    const up = Number(c.close) >= Number(c.open);
    out.push({
      time: (Date.parse(c.opened_at) / 1000) as Time,
      value: v,
      color: up ? COLORS.up : COLORS.down,
    });
  }
  return out;
}
