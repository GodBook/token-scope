export type ViewName = "dashboard" | "records" | "models" | "export";

export type Model = {
  id: string;
  name: string;
  provider: string;
  color: string;
  active: boolean;
  createdAt: string;
};

export type UsageRecord = {
  id: string;
  date: string;
  modelId: string;
  tokens: number;
  notes: string;
  createdAt: string;
};

export type TokenUnit = "1" | "M" | "B";
export type DisplayTokenUnit = "auto" | "1" | "M" | "B";

export type AppData = {
  models: Model[];
  records: UsageRecord[];
};

export type DateRange = {
  from: string;
  to: string;
};

export type Stats = {
  total: number;
  average: number;
  peakDate: string;
  peakTokens: number;
  activeModels: number;
  daily: Array<{ date: string; tokens: number }>;
  byModel: Array<{ modelId: string; name: string; provider: string; color: string; tokens: number; share: number; days: number }>;
};
