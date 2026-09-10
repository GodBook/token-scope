import ExcelJS from "exceljs";
import { save } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import type { AppData, DateRange } from "./types";
import { buildStats, formatNumber, inRange } from "./data";
import { isNativeDesktop } from "./native";

export const exportWorkbook = async (data: AppData, range: DateRange, selectedModels: string[] = []) => {
  const records = data.records.filter((record) => inRange(record.date, range) && (selectedModels.length === 0 || selectedModels.includes(record.modelId)));
  const modelMap = new Map(data.models.map((model) => [model.id, model]));
  const stats = buildStats(data.records, data.models, range, selectedModels);
  const workbook = new ExcelJS.Workbook();
  workbook.creator = "Token 统计器";
  workbook.created = new Date();

  const detail = workbook.addWorksheet("使用明细");
  detail.columns = [
    { header: "日期", key: "date", width: 15 }, { header: "模型", key: "model", width: 22 },
    { header: "供应商", key: "provider", width: 16 }, { header: "Token 数量", key: "tokens", width: 18 },
    { header: "备注", key: "notes", width: 30 },
  ];
  records.sort((a, b) => b.date.localeCompare(a.date)).forEach((record) => {
    const model = modelMap.get(record.modelId);
    detail.addRow({ date: record.date, model: model?.name ?? "未知模型", provider: model?.provider ?? "", tokens: record.tokens, notes: record.notes });
  });

  const daily = workbook.addWorksheet("按日汇总");
  daily.columns = [{ header: "日期", key: "date", width: 15 }, { header: "总 Token", key: "tokens", width: 18 }, { header: "记录数", key: "count", width: 12 }];
  stats.daily.forEach((item) => daily.addRow({ date: item.date, tokens: item.tokens, count: records.filter((record) => record.date === item.date).length }));

  const byModel = workbook.addWorksheet("按模型汇总");
  byModel.columns = [{ header: "模型", key: "model", width: 22 }, { header: "供应商", key: "provider", width: 16 }, { header: "Token 总量", key: "tokens", width: 18 }, { header: "占比", key: "share", width: 12 }, { header: "记录天数", key: "days", width: 12 }];
  stats.byModel.forEach((item) => byModel.addRow({ model: item.name, provider: item.provider, tokens: item.tokens, share: item.share, days: item.days }));

  workbook.worksheets.forEach((sheet) => {
    sheet.views = [{ state: "frozen", ySplit: 1 }];
    const header = sheet.getRow(1);
    header.font = { bold: true, color: { argb: "FFFFFFFF" } };
    header.fill = { type: "pattern", pattern: "solid", fgColor: { argb: "FF14253D" } };
    header.alignment = { vertical: "middle" };
    header.height = 26;
    sheet.eachRow((row, index) => {
      if (index > 1) row.alignment = { vertical: "middle" };
      row.getCell("tokens").numFmt = "#,##0";
      if (sheet.name === "按模型汇总") row.getCell("share").numFmt = "0.0%";
    });
    sheet.autoFilter = { from: "A1", to: `${String.fromCharCode(64 + sheet.columnCount)}1` };
  });

  const buffer = await workbook.xlsx.writeBuffer();
  const fileName = `Token使用统计_${range.from}至${range.to}.xlsx`;
  if (isNativeDesktop()) {
    const destination = await save({ defaultPath: fileName, filters: [{ name: "Excel 工作簿", extensions: ["xlsx"] }] });
    if (!destination) throw new Error("EXPORT_CANCELLED");
    await writeFile(destination, new Uint8Array(buffer as ArrayBuffer));
    return { fileName: destination.split(/[\\/]/).pop() ?? fileName, count: records.length, summary: `${formatNumber(stats.total)} tokens` };
  }
  const blob = new Blob([buffer], { type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = fileName;
  anchor.click();
  URL.revokeObjectURL(url);
  return { fileName, count: records.length, summary: `${formatNumber(stats.total)} tokens` };
};
