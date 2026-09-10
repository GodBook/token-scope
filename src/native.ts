import { invoke } from "@tauri-apps/api/core";
import type { AppData, Model, UsageRecord } from "./types";

type NativeModel = { id: string; name: string; provider?: string | null; color?: string | null; isActive: boolean; createdAt: string };
type NativeRecord = { id: string; usageDate: string; modelId: string; modelName: string; provider?: string | null; color?: string | null; tokenCount: number; notes?: string | null; createdAt: string };
type NativePage = { items: NativeRecord[]; total: number; page: number; pageSize: number };

export const isNativeDesktop = () => typeof window !== "undefined" && Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);

const mapModel = (model: NativeModel): Model => ({ id: model.id, name: model.name, provider: model.provider ?? "", color: model.color ?? "#e5a84b", active: model.isActive, createdAt: model.createdAt });
const mapRecord = (record: NativeRecord): UsageRecord => ({ id: record.id, date: record.usageDate, modelId: record.modelId, tokens: record.tokenCount, notes: record.notes ?? "", createdAt: record.createdAt });

export const loadNativeData = async (): Promise<AppData> => {
  const [nativeModels, firstPage] = await Promise.all([
    invoke<NativeModel[]>("list_models", { includeInactive: true }),
    invoke<NativePage>("list_usage_records", { filter: {}, sort: { field: "usage_date", direction: "desc" }, page: { page: 1, pageSize: 1000 } }),
  ]);
  const pages = [firstPage];
  const totalPages = Math.ceil(firstPage.total / firstPage.pageSize);
  for (let page = 2; page <= totalPages; page += 1) pages.push(await invoke<NativePage>("list_usage_records", { filter: {}, sort: { field: "usage_date", direction: "desc" }, page: { page, pageSize: 1000 } }));
  return { models: nativeModels.map(mapModel), records: pages.flatMap((page) => page.items).map(mapRecord) };
};

export const nativeSaveRecord = (record: UsageRecord, overwrite: boolean) => invoke<NativeRecord>("save_usage_record", { usageDate: record.date, modelId: record.modelId, tokenCount: record.tokens, notes: record.notes || null, overwrite });
export const nativeSaveUsageRecordsBatch = (records: UsageRecord[], overwrite: boolean) => invoke<NativeRecord[]>("save_usage_records_batch", {
  records: records.map((record) => ({ usageDate: record.date, modelId: record.modelId, tokenCount: record.tokens, notes: record.notes || null })),
  overwrite,
}).then((saved) => saved.map(mapRecord));
export const nativeDeleteRecord = (id: string) => invoke<boolean>("delete_usage_record", { id });
export const nativeDeleteModel = (id: string) => invoke<boolean>("delete_model", { id });
export const nativeCreateModel = (model: Model) => invoke<NativeModel>("create_model", { name: model.name, provider: model.provider || null, color: model.color || null });
export const nativeUpdateModel = (model: Model) => invoke<NativeModel>("update_model", { id: model.id, name: model.name, provider: model.provider || null, color: model.color || null });
export const nativeSetModelActive = (model: Model) => invoke<NativeModel>("set_model_active", { id: model.id, active: model.active });
