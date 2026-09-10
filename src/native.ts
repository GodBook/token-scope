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

export type UpdateInfo = {
  hasUpdate: boolean;
  currentVersion: string;
  latestVersion: string;
  releaseName: string;
  releaseNotes: string;
  releaseDate: string;
  downloadUrl?: string | null;
  assetName?: string | null;
  assetSize?: number | null;
  releaseUrl: string;
};

export type DownloadProgress = {
  percentage: number;
  downloaded: number;
  total: number;
};

export type BackupResult = {
  success: boolean;
  backupPath: string;
  message: string;
};

export type AppMetadata = {
  version: string;
  appDataDir: string;
  databasePath: string;
  backupCount: number;
};

export const nativeCheckAppUpdate = async (): Promise<UpdateInfo> => {
  if (!isNativeDesktop()) {
    try {
      const res = await fetch("https://api.github.com/repos/GodBook/token-scope/releases/latest");
      if (res.status === 404) {
        return {
          hasUpdate: false,
          currentVersion: "0.1.0",
          latestVersion: "0.1.0",
          releaseName: "暂无发布记录",
          releaseNotes: "当前 GitHub 仓库尚未创建任何 Release 发布版本。",
          releaseDate: "",
          releaseUrl: "https://github.com/GodBook/token-scope/releases",
        };
      }
      if (res.ok) {
        const data = await res.json();
        const tag = (data.tag_name || "").replace(/^[vV]/, "");
        const asset = data.assets?.find((a: { name: string }) => a.name.endsWith(".exe") || a.name.endsWith(".msi")) || data.assets?.[0];
        return {
          hasUpdate: tag !== "0.1.0" && tag !== "",
          currentVersion: "0.1.0",
          latestVersion: tag || "0.1.0",
          releaseName: data.name || data.tag_name || "最新版本",
          releaseNotes: data.body || "",
          releaseDate: data.published_at || "",
          downloadUrl: asset?.browser_download_url,
          assetName: asset?.name,
          assetSize: asset?.size,
          releaseUrl: data.html_url || "https://github.com/GodBook/token-scope/releases",
        };
      }
    } catch {
      // ignore
    }
    return {
      hasUpdate: false,
      currentVersion: "0.1.0",
      latestVersion: "0.1.0",
      releaseName: "当前版本已是最新",
      releaseNotes: "当前运行在浏览器预览模式下。",
      releaseDate: "",
      releaseUrl: "https://github.com/GodBook/token-scope/releases",
    };
  }
  return invoke<UpdateInfo>("check_app_update");
};

export const nativeDownloadAndInstallUpdate = async (downloadUrl: string, assetName: string): Promise<string> => {
  if (!isNativeDesktop()) {
    window.open(downloadUrl, "_blank");
    return "已在浏览器中打开下载链接";
  }
  return invoke<string>("download_and_install_update", { downloadUrl, assetName });
};

export const nativeBackupDatabaseNow = async (): Promise<BackupResult> => {
  if (!isNativeDesktop()) {
    return { success: true, backupPath: "localStorage", message: "预览模式数据已存储在浏览器缓存中" };
  }
  return invoke<BackupResult>("backup_database_now");
};

export const nativeGetAppInfo = async (): Promise<AppMetadata> => {
  if (!isNativeDesktop()) {
    return {
      version: "0.1.0",
      appDataDir: "浏览器本地环境 (localStorage)",
      databasePath: "localStorage:token_scope_data",
      backupCount: 0,
    };
  }
  return invoke<AppMetadata>("get_app_info");
};

export const listenUpdateProgress = (callback: (progress: DownloadProgress) => void) => {
  if (!isNativeDesktop()) return () => {};
  let unlistenFn: (() => void) | null = null;
  import("@tauri-apps/api/event").then(({ listen }) => {
    listen<DownloadProgress>("update-download-progress", (event) => {
      callback(event.payload);
    }).then((fn) => {
      unlistenFn = fn;
    });
  });
  return () => {
    if (unlistenFn) unlistenFn();
  };
};
