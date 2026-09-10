import { describe, expect, it } from "vitest";
import { buildStats, displayTokenUnitLabel, formatCompact, formatPercent, formatTokenValue, getDateRange, inRange, isShareVisible, parseTokenInput, shiftDate, todayString } from "./data";
import type { Model, UsageRecord } from "./types";

const models: Model[] = [
  { id: "a", name: "模型 A", provider: "A", color: "#e5a84b", active: true, createdAt: "2026-01-01" },
  { id: "b", name: "模型 B", provider: "B", color: "#5bb9a4", active: true, createdAt: "2026-01-01" },
];
const records: UsageRecord[] = [
  { id: "1", date: "2026-08-01", modelId: "a", tokens: 100, notes: "", createdAt: "" },
  { id: "2", date: "2026-08-02", modelId: "a", tokens: 300, notes: "", createdAt: "" },
  { id: "3", date: "2026-08-02", modelId: "b", tokens: 600, notes: "", createdAt: "" },
];

describe("统计逻辑", () => {
  it("按日汇总并补齐没有记录的日期", () => {
    const stats = buildStats(records, models, { from: "2026-08-01", to: "2026-08-03" });
    expect(stats.total).toBe(1000);
    expect(stats.average).toBeCloseTo(333.333);
    expect(stats.daily).toEqual([{ date: "2026-08-01", tokens: 100 }, { date: "2026-08-02", tokens: 900 }, { date: "2026-08-03", tokens: 0 }]);
    expect(stats.peakDate).toBe("2026-08-02");
    expect(stats.byModel[0].name).toBe("模型 B");
  });

  it("支持模型筛选和日期边界", () => {
    const stats = buildStats(records, models, { from: "2026-08-01", to: "2026-08-02" }, ["a"]);
    expect(stats.total).toBe(400);
    expect(inRange("2026-08-01", { from: "2026-08-01", to: "2026-08-02" })).toBe(true);
    expect(inRange("2026-08-03", { from: "2026-08-01", to: "2026-08-02" })).toBe(false);
  });

  it("生成快捷日期范围和可读数值", () => {
    const today = todayString();
    expect(shiftDate("2026-08-25", -1)).toBe("2026-08-24");
    expect(getDateRange("7d", records)).toEqual({ from: shiftDate(today, -6), to: today });
    expect(getDateRange("14d", records)).toEqual({ from: shiftDate(today, -13), to: today });
    expect(getDateRange("1d", records)).toEqual({ from: today, to: today });
    expect(formatCompact(1250000)).toBe("1.3M");
  });

  it("按 1、M、B 单位解析 Token，并拒绝超出安全整数的输入", () => {
    expect(parseTokenInput("1250000", "1")).toBe(1250000);
    expect(parseTokenInput("1.5", "M")).toBe(1500000);
    expect(parseTokenInput("33.16", "M")).toBe(33160000);
    expect(parseTokenInput("2", "B")).toBe(2000000000);
    expect(parseTokenInput("1.5", "1")).toBeNull();
    expect(parseTokenInput("9007199255", "M")).toBeNull();
  });

  it("按自定义显示单位格式化图表数值", () => {
    expect(formatTokenValue(45010000, "M")).toBe("45.01M");
    expect(formatTokenValue(45010000, "B")).toBe("0.045B");
    expect(formatTokenValue(45010000, "1")).toBe("45,010,000");
    expect(formatTokenValue(45010000, "auto", true)).toBe("45M");
    expect(displayTokenUnitLabel("auto")).toBe("自动");
  });

  it("占比图例隐藏格式化后为 0.0% 的项目", () => {
    expect(formatPercent(0)).toBe("0.0%");
    expect(isShareVisible(0)).toBe(false);
    expect(formatPercent(0.0004)).toBe("0.0%");
    expect(isShareVisible(0.0004)).toBe(false);
    expect(formatPercent(0.0005)).toBe("0.1%");
    expect(isShareVisible(0.0005)).toBe(true);
  });
});
