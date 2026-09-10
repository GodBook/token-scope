import type { AppData, DateRange, DisplayTokenUnit, Model, Stats, TokenUnit, UsageRecord } from "./types";

const STORAGE_KEY = "token-statistics-data-v1";
const colors = ["#e5a84b", "#5bb9a4", "#7986e8", "#e57f74", "#ae79c7", "#66a9ce"];

const localDate = (date: Date) => {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
};

export const todayString = () => localDate(new Date());

export const shiftDate = (dateString: string, amount: number) => {
  const date = new Date(`${dateString}T12:00:00`);
  date.setDate(date.getDate() + amount);
  return localDate(date);
};

const createSeedData = (): AppData => {
  const now = new Date().toISOString();
  const models: Model[] = [
    { id: "gpt-5", name: "GPT-5", provider: "OpenAI", color: colors[0], active: true, createdAt: now },
    { id: "claude-sonnet-4", name: "Claude Sonnet 4", provider: "Anthropic", color: colors[1], active: true, createdAt: now },
    { id: "gemini-2-5-pro", name: "Gemini 2.5 Pro", provider: "Google", color: colors[2], active: true, createdAt: now },
    { id: "deepseek-v3", name: "DeepSeek V3", provider: "DeepSeek", color: colors[3], active: true, createdAt: now },
  ];
  const base = new Date();
  base.setDate(base.getDate() - 24);
  const values = [
    [680000, 410000, 270000, 0], [820000, 520000, 190000, 86000], [490000, 360000, 0, 120000],
    [1040000, 710000, 260000, 92000], [760000, 440000, 210000, 0], [920000, 610000, 180000, 90000],
    [560000, 290000, 160000, 78000], [1190000, 820000, 240000, 130000], [890000, 550000, 220000, 0],
    [730000, 430000, 170000, 95000], [970000, 640000, 250000, 80000], [1110000, 730000, 310000, 0],
  ];
  const records: UsageRecord[] = [];
  values.forEach((row, rowIndex) => {
    const date = new Date(base);
    date.setDate(base.getDate() + rowIndex * 2);
    row.forEach((tokens, modelIndex) => {
      if (tokens === 0) return;
      records.push({
        id: `seed-${rowIndex}-${modelIndex}`,
        date: localDate(date),
        modelId: models[modelIndex].id,
        tokens,
        notes: rowIndex % 4 === 0 && modelIndex === 0 ? "项目冲刺" : "",
        createdAt: now,
      });
    });
  });
  return { models, records };
};

const isTauri = () => typeof window !== "undefined" && Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);

export const loadData = (): AppData => {
  if (typeof window === "undefined") return createSeedData();
  const stored = window.localStorage.getItem(STORAGE_KEY);
  if (!stored) {
    const seed = createSeedData();
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(seed));
    return seed;
  }
  try {
    return JSON.parse(stored) as AppData;
  } catch {
    return createSeedData();
  }
};

export const saveData = (data: AppData) => {
  if (typeof window !== "undefined") window.localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
};

export const canUseNativeStorage = isTauri;

export const getDateRange = (preset: "1d" | "7d" | "14d" | "30d" | "90d" | "all", records: UsageRecord[]): DateRange => {
  const to = todayString();
  if (preset === "1d") return { from: to, to };
  if (preset === "all") {
    const dates = records.map((record) => record.date).sort();
    return { from: dates[0] ?? shiftDate(to, -29), to };
  }
  const days = preset === "7d" ? 6 : preset === "14d" ? 13 : preset === "90d" ? 89 : 29;
  return { from: shiftDate(to, -days), to };
};

export const inRange = (date: string, range: DateRange) => date >= range.from && date <= range.to;

export const buildStats = (records: UsageRecord[], models: Model[], range: DateRange, selectedModels: string[] = []): Stats => {
  const filtered = records.filter((record) => inRange(record.date, range) && (selectedModels.length === 0 || selectedModels.includes(record.modelId)));
  const total = filtered.reduce((sum, record) => sum + record.tokens, 0);
  const daily: Stats["daily"] = [];
  let cursor = range.from;
  while (cursor <= range.to) {
    daily.push({ date: cursor, tokens: filtered.filter((record) => record.date === cursor).reduce((sum, record) => sum + record.tokens, 0) });
    cursor = shiftDate(cursor, 1);
  }
  const byModel = models
    .map((model) => {
      const modelRecords = filtered.filter((record) => record.modelId === model.id);
      const tokens = modelRecords.reduce((sum, record) => sum + record.tokens, 0);
      return { modelId: model.id, name: model.name, provider: model.provider, color: model.color, tokens, share: total ? tokens / total : 0, days: modelRecords.length };
    })
    .filter((item) => item.tokens > 0)
    .sort((a, b) => b.tokens - a.tokens);
  const peak = daily.reduce((best, item) => item.tokens > best.tokens ? item : best, { date: "", tokens: 0 });
  const dayCount = Math.max(daily.length, 1);
  return { total, average: total / dayCount, peakDate: peak.date, peakTokens: peak.tokens, activeModels: byModel.length, daily, byModel };
};

export const formatCompact = (value: number) => {
  if (value >= 1000000) return `${(value / 1000000).toFixed(value >= 10000000 ? 0 : 1)}M`;
  if (value >= 1000) return `${(value / 1000).toFixed(value >= 100000 ? 0 : 1)}K`;
  return String(Math.round(value));
};

export const formatNumber = (value: number) => new Intl.NumberFormat("zh-CN").format(Math.round(value));

export const formatDateLabel = (date: string) => {
  const [, month, day] = date.split("-");
  return `${Number(month)}月${Number(day)}日`;
};

export const formatPercent = (value: number) => `${(value * 100).toFixed(value < 0.1 ? 1 : 0)}%`;

/**
 * 判断占比是否值得在占比图例中展示。
 * 与 formatPercent 使用同一套舍入规则，避免图例出现实际有数据但显示为 0.0% 的项目。
 */
export const isShareVisible = (value: number) => formatPercent(value) !== "0.0%";

export const makeId = (prefix: string) => `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;

const tokenUnitDecimals: Record<TokenUnit, number> = { "1": 0, M: 6, B: 9 };
const maxSafeIntegerBigInt = BigInt(Number.MAX_SAFE_INTEGER);

export const parseTokenInput = (value: string, unit: TokenUnit): number | null => {
  const normalized = value.trim().replace(/,/g, "");
  if (!/^\d+(?:\.\d+)?$/.test(normalized)) return null;
  const [whole, fraction = ""] = normalized.split(".");
  const decimals = tokenUnitDecimals[unit];
  if (fraction.length > decimals && /[1-9]/.test(fraction.slice(decimals))) return null;
  const scaledFraction = fraction.slice(0, decimals).padEnd(decimals, "0");
  const scaled = BigInt(`${whole}${scaledFraction}`);
  return scaled > 0n && scaled <= maxSafeIntegerBigInt ? Number(scaled) : null;
};

export const formatTokenValue = (value: number, unit: DisplayTokenUnit, compactAuto = false) => {
  if (unit === "auto") return compactAuto ? formatCompact(value) : formatNumber(value);
  const divisor = unit === "1" ? 1 : unit === "M" ? 1_000_000 : 1_000_000_000;
  const decimals = unit === "1" ? 0 : unit === "M" ? 2 : 3;
  const scaled = value / divisor;
  const formatted = new Intl.NumberFormat("zh-CN", { maximumFractionDigits: decimals }).format(scaled);
  return unit === "1" ? formatted : `${formatted}${unit}`;
};

export const displayTokenUnitLabel = (unit: DisplayTokenUnit) => unit === "auto" ? "自动" : unit === "1" ? "1" : unit;
