import React, { useEffect, useMemo, useRef, useState } from "react";
import * as echarts from "echarts";
import {
  BarChart3,
  CalendarDays,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CircleHelp,
  Database,
  Download,
  ExternalLink,
  FileSpreadsheet,
  Filter,
  HardDriveDownload,
  LayoutDashboard,
  Menu,
  Moon,
  MoreHorizontal,
  Pencil,
  Plus,
  RefreshCw,
  Search,
  Settings,
  ShieldCheck,
  SlidersHorizontal,
  Sun,
  Trash2,
  Upload,
  X,
  Zap,
} from "lucide-react";
import "./App.css";
import {
  buildStats,
  formatCompact,
  formatDateLabel,
  formatNumber,
  formatPercent,
  formatTokenValue,
  getDateRange,
  inRange,
  isShareVisible,
  loadData,
  makeId,
  parseTokenInput,
  saveData,
  shiftDate,
  todayString,
} from "./data";
import { exportWorkbook } from "./export";
import {
  isNativeDesktop,
  listenUpdateProgress,
  loadNativeData,
  nativeBackupDatabaseNow,
  nativeCheckAppUpdate,
  nativeCreateModel,
  nativeDeleteModel,
  nativeDeleteRecord,
  nativeDownloadAndInstallUpdate,
  nativeGetAppInfo,
  nativeSaveRecord,
  nativeSaveUsageRecordsBatch,
  nativeSetModelActive,
  nativeUpdateModel,
  type AppMetadata,
  type DownloadProgress,
  type UpdateInfo,
} from "./native";
import type {
  AppData,
  DateRange,
  DisplayTokenUnit,
  Model,
  Stats,
  TokenUnit,
  UsageRecord,
  ViewName,
} from "./types";

const navItems: Array<{
  id: ViewName;
  label: string;
  icon: typeof LayoutDashboard;
}> = [
  { id: "dashboard", label: "仪表盘", icon: LayoutDashboard },
  { id: "records", label: "使用记录", icon: Database },
  { id: "models", label: "模型管理", icon: SlidersHorizontal },
  { id: "export", label: "导入导出", icon: FileSpreadsheet },
];

type DashboardPreset = "1d" | "7d" | "14d" | "30d" | "90d" | "all";
type CustomRangeMode = "day" | "range";

const datePresets: Array<{ id: DashboardPreset; label: string }> = [
  { id: "1d", label: "单日" },
  { id: "7d", label: "近 1 周" },
  { id: "14d", label: "近 2 周" },
  { id: "30d", label: "近 1 月" },
  { id: "90d", label: "近 3 月" },
  { id: "all", label: "全部" },
];

const dateFormat = (date: string) => {
  const [year, month, day] = date.split("-");
  return `${year} 年 ${Number(month)} 月 ${Number(day)} 日`;
};

const listDates = (from: string, to: string) => {
  if (!from || !to || from > to) return [] as string[];
  const dates: string[] = [];
  let cursor = from;
  while (cursor <= to && dates.length <= 10_000) {
    dates.push(cursor);
    cursor = shiftDate(cursor, 1);
  }
  return dates;
};

const Chart = ({
  option,
  className = "",
}: {
  option: echarts.EChartsOption;
  className?: string;
}) => {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!ref.current) return;
    const chart = echarts.init(ref.current);
    chart.setOption(option);
    const handleResize = () => chart.resize();
    window.addEventListener("resize", handleResize);
    return () => {
      window.removeEventListener("resize", handleResize);
      chart.dispose();
    };
  }, [option]);
  return (
    <div ref={ref} className={`chart ${className}`} aria-label="数据图表" />
  );
};

function App() {
  const [view, setView] = useState<ViewName>("dashboard");
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const initialData = useMemo(() => loadData(), []);
  const [data, setData] = useState<AppData>(initialData);
  const [range, setRange] = useState<DateRange>(() =>
    getDateRange("30d", initialData.records),
  );
  const [preset, setPreset] = useState<DashboardPreset | "custom">("30d");
  const [selectedModels, setSelectedModels] = useState<string[]>([]);
  const [query, setQuery] = useState("");
  const [theme, setTheme] = useState<"light" | "dark">("light");
  const [displayTokenUnit, setDisplayTokenUnit] =
    useState<DisplayTokenUnit>("auto");
  const [recordModal, setRecordModal] = useState<{
    record?: UsageRecord;
  } | null>(null);
  const [modelModal, setModelModal] = useState<{ model?: Model } | null>(null);
  const [updateModalOpen, setUpdateModalOpen] = useState(false);
  const [hasNewVersion, setHasNewVersion] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);
  useEffect(() => {
    if (!isNativeDesktop()) return;
    loadNativeData()
      .then(setData)
      .catch(() => setToast("本地数据库暂时无法读取，已使用预览数据"));
  }, []);
  useEffect(() => {
    // 启动时在后台静默检查一次是否有新版本
    nativeCheckAppUpdate()
      .then((info) => {
        if (info.hasUpdate) {
          setHasNewVersion(true);
        }
      })
      .catch(() => {});
  }, []);
  useEffect(() => {
    if (!toast) return;
    const timer = window.setTimeout(() => setToast(null), 2800);
    return () => window.clearTimeout(timer);
  }, [toast]);
  useEffect(() => {
    saveData(data);
  }, [data]);

  const stats = useMemo(
    () => buildStats(data.records, data.models, range, selectedModels),
    [data, range, selectedModels],
  );
  const filteredRecords = useMemo(
    () =>
      data.records
        .filter(
          (record) =>
            inRange(record.date, range) &&
            (selectedModels.length === 0 ||
              selectedModels.includes(record.modelId)),
        )
        .filter((record) => {
          const model = data.models.find((item) => item.id === record.modelId);
          return (
            !query ||
            model?.name.toLowerCase().includes(query.toLowerCase()) ||
            record.notes.toLowerCase().includes(query.toLowerCase())
          );
        })
        .sort((a, b) => b.date.localeCompare(a.date)),
    [data, range, selectedModels, query],
  );

  const changePreset = (nextPreset: DashboardPreset) => {
    setPreset(nextPreset);
    setRange(getDateRange(nextPreset, data.records));
  };
  const applyCustomRange = (nextRange: DateRange) => {
    if (!nextRange.from || !nextRange.to || nextRange.from > nextRange.to) {
      setToast("请选择有效的日期范围");
      return false;
    }
    setPreset("custom");
    setRange(nextRange);
    return true;
  };
  const moveRange = (amount: number) => {
    // 以包含首尾日期的天数作为步长；单日范围应前后移动一天。
    const span = Math.max(
      Math.round(
        (new Date(`${range.to}T12:00:00`).getTime() -
          new Date(`${range.from}T12:00:00`).getTime()) /
          86400000,
      ) + 1,
      1,
    );
    setPreset("custom");
    setRange({
      from: shiftDate(range.from, amount * span),
      to: shiftDate(range.to, amount * span),
    });
  };
  const handleSaveRecord = (record: UsageRecord, overwrite: boolean) => {
    const duplicate = data.records.find(
      (item) =>
        item.date === record.date &&
        item.modelId === record.modelId &&
        item.id !== record.id,
    );
    if (duplicate && !overwrite) {
      setToast("这一天已有同模型记录，请确认覆盖");
      return false;
    }
    setData((current) => ({
      ...current,
      records: [
        ...current.records.filter(
          (item) => item.id !== record.id && item.id !== duplicate?.id,
        ),
        record,
      ],
    }));
    if (isNativeDesktop())
      nativeSaveRecord(record, Boolean(duplicate || overwrite))
        .then((saved) =>
          setData((current) => ({
            ...current,
            records: current.records.map((item) =>
              item.id === record.id ? { ...item, id: saved.id } : item,
            ),
          })),
        )
        .catch(() => setToast("记录已更新，但写入本地数据库失败"));
    setRecordModal(null);
    setToast("记录已保存");
    return true;
  };
  const handleBatchImport = async (
    records: UsageRecord[],
    overwrite: boolean,
  ) => {
    if (isNativeDesktop()) {
      const saved = await nativeSaveUsageRecordsBatch(records, overwrite);
      const keys = new Set(
        saved.map((record) => `${record.date}::${record.modelId}`),
      );
      setData((current) => ({
        ...current,
        records: [
          ...current.records.filter(
            (record) => !keys.has(`${record.date}::${record.modelId}`),
          ),
          ...saved,
        ],
      }));
    } else {
      const keys = new Set(
        records.map((record) => `${record.date}::${record.modelId}`),
      );
      setData((current) => ({
        ...current,
        records: [
          ...current.records.filter(
            (record) => !keys.has(`${record.date}::${record.modelId}`),
          ),
          ...records,
        ],
      }));
    }
    setToast(`已批量导入 ${records.length} 天记录`);
  };
  const handleDelete = (record: UsageRecord) => {
    if (!window.confirm(`确定删除 ${record.date} 的这条记录吗？`)) return;
    setData((current) => ({
      ...current,
      records: current.records.filter((item) => item.id !== record.id),
    }));
    setToast("记录已删除");
    if (isNativeDesktop())
      nativeDeleteRecord(record.id).catch(() =>
        setToast("记录已删除，但本地数据库同步失败"),
      );
  };
  const handleSaveModel = (model: Model) => {
    setData((current) => ({
      ...current,
      models: current.models.some((item) => item.id === model.id)
        ? current.models.map((item) => (item.id === model.id ? model : item))
        : [...current.models, model],
    }));
    if (isNativeDesktop()) {
      if (data.models.some((item) => item.id === model.id))
        nativeUpdateModel(model).catch(() =>
          setToast("界面已更新，但模型未写入本地数据库"),
        );
      else
        nativeCreateModel(model)
          .then((created) =>
            setData((current) => ({
              ...current,
              models: current.models.map((item) =>
                item.id === model.id ? { ...item, id: created.id } : item,
              ),
            })),
          )
          .catch(() => setToast("界面已更新，但模型未写入本地数据库"));
    }
    setModelModal(null);
    setToast("模型信息已保存");
  };
  const handleDeleteModel = async (model: Model, recordCount: number) => {
    if (recordCount > 0) {
      setToast("模型已有历史记录，不能删除，请改用停用");
      return;
    }
    if (!window.confirm(`确定删除模型“${model.name}”吗？此操作无法撤销。`))
      return;
    try {
      if (isNativeDesktop()) await nativeDeleteModel(model.id);
      setData((current) => ({
        ...current,
        models: current.models.filter((item) => item.id !== model.id),
      }));
      setSelectedModels((current) => current.filter((id) => id !== model.id));
      setToast("模型已删除");
    } catch (error) {
      const appError = error as { code?: string; message?: string };
      setToast(
        appError.code === "MODEL_IN_USE"
          ? "模型已有历史记录，不能删除，请改用停用"
          : appError.message || "删除模型失败，请重试",
      );
    }
  };
  const handleExport = async (scope: "current" | "all" = "current") => {
    setExporting(true);
    const exportRange =
      scope === "all" ? getDateRange("all", data.records) : range;
    const exportModels = scope === "all" ? [] : selectedModels;
    try {
      const result = await exportWorkbook(data, exportRange, exportModels);
      setToast(`已导出 ${result.fileName}`);
    } catch (error) {
      if (error instanceof Error && error.message === "EXPORT_CANCELLED")
        return;
      setToast("导出失败，请重试");
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="app-shell">
      <aside className={`sidebar ${sidebarOpen ? "open" : ""}`}>
        <div className="brand-lockup">
          <div className="brand-mark">
            <Zap size={17} strokeWidth={2.8} />
          </div>
          <div>
            <div className="brand-name">
              Token<span>Scope</span>
            </div>
            <div className="brand-caption">LOCAL ANALYTICS</div>
          </div>
        </div>
        <div className="sidebar-section-label">工作区</div>
        <nav className="main-nav" aria-label="主导航">
          {navItems.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              className={`nav-item ${view === id ? "active" : ""}`}
              onClick={() => {
                setView(id);
                setSidebarOpen(false);
              }}
            >
              <Icon size={18} />
              <span>{label}</span>
              {id === "records" && (
                <span className="nav-count">{data.records.length}</span>
              )}
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="storage-status">
            <span className="status-dot" />
            <div>
              <strong>本地存储</strong>
              <span>数据仅保存在此设备</span>
            </div>
          </div>
          <button
            className="nav-item muted"
            onClick={() => setUpdateModalOpen(true)}
            style={{ position: "relative" }}
          >
            <Settings size={18} />
            <span>设置与更新</span>
            {hasNewVersion && <span className="update-dot" style={{ marginLeft: "auto" }} />}
          </button>
          <button
            className="app-version-btn"
            onClick={() => setUpdateModalOpen(true)}
            title="点击检查更新"
          >
            <span>v1.0.0 · Windows 桌面版</span>
            {hasNewVersion ? (
              <span className="update-dot" title="有新版本可用" />
            ) : (
              <span style={{ textDecoration: "underline", opacity: 0.75 }}>检查更新</span>
            )}
          </button>
        </div>
      </aside>
      <main className="main-content">
        <header className="topbar">
          <button
            className="mobile-menu"
            aria-label="打开菜单"
            onClick={() => setSidebarOpen((open) => !open)}
          >
            <Menu size={20} />
          </button>
          <div className="breadcrumb">
            <span>工作区</span>
            <ChevronRight size={14} />
            <strong>{navItems.find((item) => item.id === view)?.label}</strong>
          </div>
          <div className="topbar-actions">
            <button
              className="icon-button"
              aria-label="设置与更新"
              title="设置与更新"
              onClick={() => setUpdateModalOpen(true)}
            >
              <Settings size={18} />
              {hasNewVersion && <span className="topbar-update-dot" />}
            </button>
            <button
              className="icon-button"
              aria-label="帮助"
              title="帮助"
              onClick={() => setToast("所有数据都保存在本地，不会上传")}
            >
              <CircleHelp size={18} />
            </button>
            <button
              className="icon-button"
              aria-label="切换主题"
              title="切换主题"
              onClick={() =>
                setTheme((current) => (current === "light" ? "dark" : "light"))
              }
            >
              {theme === "light" ? <Moon size={18} /> : <Sun size={18} />}
            </button>
            <div className="profile-chip">
              <span className="profile-avatar">你</span>
              <span>我的工作区</span>
              <ChevronDown size={14} />
            </div>
          </div>
        </header>
        {view === "dashboard" && (
          <DashboardView
            stats={stats}
            range={range}
            preset={preset}
            changePreset={changePreset}
            moveRange={moveRange}
            applyCustomRange={applyCustomRange}
            selectedModels={selectedModels}
            setSelectedModels={setSelectedModels}
            models={data.models}
            displayTokenUnit={displayTokenUnit}
            setDisplayTokenUnit={setDisplayTokenUnit}
            onNew={() => setRecordModal({})}
            onExport={handleExport}
            exporting={exporting}
          />
        )}
        {view === "records" && (
          <RecordsView
            records={filteredRecords}
            models={data.models}
            range={range}
            query={query}
            setQuery={setQuery}
            onNew={() => setRecordModal({})}
            onEdit={(record) => setRecordModal({ record })}
            onDelete={handleDelete}
            onExport={handleExport}
            exporting={exporting}
          />
        )}
        {view === "models" && (
          <ModelsView
            models={data.models}
            records={data.records}
            onNew={() => setModelModal({})}
            onEdit={(model) => setModelModal({ model })}
            onToggle={(model) => {
              const next = { ...model, active: !model.active };
              handleSaveModel(next);
              if (isNativeDesktop())
                nativeSetModelActive(next).catch(() =>
                  setToast("模型状态已更新，但本地数据库同步失败"),
                );
            }}
            onDelete={handleDeleteModel}
          />
        )}
        {view === "export" && (
          <ExportView
            range={range}
            setRange={(next) => {
              setPreset("custom");
              setRange(next);
            }}
            stats={stats}
            models={data.models}
            records={data.records}
            onBatchImport={handleBatchImport}
            onExport={() => handleExport("current")}
            onExportAll={() => handleExport("all")}
            exporting={exporting}
          />
        )}
      </main>
      {sidebarOpen && (
        <button
          className="sidebar-overlay"
          aria-label="关闭菜单"
          onClick={() => setSidebarOpen(false)}
        />
      )}
      {recordModal && (
        <RecordModal
          record={recordModal.record}
          models={data.models.filter((model) => model.active)}
          onClose={() => setRecordModal(null)}
          onSave={handleSaveRecord}
        />
      )}
      {modelModal && (
        <ModelModal
          model={modelModal.model}
          onClose={() => setModelModal(null)}
          onSave={handleSaveModel}
        />
      )}
      {updateModalOpen && (
        <UpdateModal
          onClose={() => setUpdateModalOpen(false)}
          onToast={setToast}
        />
      )}
      {toast && (
        <div className="toast">
          <span className="toast-indicator" />
          {toast}
          <button onClick={() => setToast(null)} aria-label="关闭提示">
            <X size={15} />
          </button>
        </div>
      )}
    </div>
  );
}

function DashboardView({
  stats,
  range,
  preset,
  changePreset,
  moveRange,
  applyCustomRange,
  selectedModels,
  setSelectedModels,
  models,
  displayTokenUnit,
  setDisplayTokenUnit,
  onNew,
  onExport,
  exporting,
}: {
  stats: Stats;
  range: DateRange;
  preset: DashboardPreset | "custom";
  changePreset: (preset: DashboardPreset) => void;
  moveRange: (amount: number) => void;
  applyCustomRange: (range: DateRange) => boolean;
  selectedModels: string[];
  setSelectedModels: (models: string[]) => void;
  models: Model[];
  displayTokenUnit: DisplayTokenUnit;
  setDisplayTokenUnit: (unit: DisplayTokenUnit) => void;
  onNew: () => void;
  onExport: () => void;
  exporting: boolean;
}) {
  const [modelMenu, setModelMenu] = useState(false);
  const [customRangeOpen, setCustomRangeOpen] = useState(false);
  const [customMode, setCustomMode] = useState<CustomRangeMode>(() =>
    range.from === range.to ? "day" : "range",
  );
  const [customFrom, setCustomFrom] = useState(range.from);
  const [customTo, setCustomTo] = useState(range.to);
  const [customDate, setCustomDate] = useState(range.from);
  const [customRangeError, setCustomRangeError] = useState("");
  useEffect(() => {
    setCustomFrom(range.from);
    setCustomTo(range.to);
    setCustomDate(range.from);
    setCustomMode(range.from === range.to ? "day" : "range");
  }, [range.from, range.to]);
  const submitCustomRange = () => {
    const nextRange =
      customMode === "day"
        ? { from: customDate, to: customDate }
        : { from: customFrom, to: customTo };
    if (!nextRange.from || !nextRange.to || nextRange.from > nextRange.to) {
      setCustomRangeError("开始日期不能晚于结束日期");
      return;
    }
    if (applyCustomRange(nextRange)) {
      setCustomRangeError("");
      setCustomRangeOpen(false);
    }
  };
  const visibleShareModels = useMemo(
    () => stats.byModel.filter((item) => isShareVisible(item.share)),
    [stats.byModel],
  );
  const trendOption = useMemo<echarts.EChartsOption>(
    () => ({
      animationDuration: 500,
      grid: { top: 38, right: 12, bottom: 30, left: 52 },
      tooltip: {
        trigger: "axis",
        backgroundColor: "#14253d",
        borderWidth: 0,
        textStyle: { color: "#fff" },
        formatter: (params) => {
          const point = Array.isArray(params)
            ? (params[0] as unknown as { axisValue: string; value: number })
            : (params as unknown as { axisValue: string; value: number });
          return `<strong>${point.axisValue}</strong><br/>${formatTokenValue(point.value, displayTokenUnit)} tokens`;
        },
      },
      xAxis: {
        type: "category",
        boundaryGap: false,
        data: stats.daily.map((item) => formatDateLabel(item.date)),
        axisLine: { lineStyle: { color: "#d8dee8" } },
        axisLabel: {
          color: "#8692a3",
          fontSize: 11,
          interval: Math.max(Math.ceil(stats.daily.length / 7) - 1, 0),
        },
      },
      yAxis: {
        type: "value",
        splitNumber: 4,
        axisLabel: {
          color: "#8692a3",
          fontSize: 11,
          formatter: (value: number) =>
            formatTokenValue(value, displayTokenUnit, true),
        },
        splitLine: { lineStyle: { color: "#edf0f4" } },
        axisLine: { show: false },
      },
      series: [
        {
          type: "line",
          smooth: 0.36,
          symbol: "circle",
          symbolSize: 6,
          data: stats.daily.map((item) => item.tokens),
          label: {
            show: true,
            position: "top",
            distance: 6,
            color: "#536579",
            fontSize: 9,
            fontWeight: 600,
            formatter: (params) => {
              const point = params as { value: number };
              return point.value > 0
                ? formatTokenValue(point.value, displayTokenUnit, true)
                : "";
            },
          },
          labelLayout: { hideOverlap: false },
          lineStyle: { color: "#e5a84b", width: 3 },
          itemStyle: { color: "#e5a84b", borderColor: "#fff", borderWidth: 2 },
          areaStyle: {
            color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [
              { offset: 0, color: "rgba(229,168,75,.25)" },
              { offset: 1, color: "rgba(229,168,75,0)" },
            ]),
          },
        },
      ],
      media: [
        {
          query: { maxWidth: 520 },
          option: {
            grid: { top: 48, right: 8, bottom: 30, left: 46 },
            series: [{ label: { rotate: 45, distance: 5, fontSize: 8 } }],
          },
        },
      ],
    }),
    [stats.daily, displayTokenUnit],
  );
  const isRotated = stats.byModel.length > 4;
  const barOption = useMemo<echarts.EChartsOption>(
    () => ({
      grid: {
        top: 32,
        right: 16,
        bottom: isRotated ? 58 : 28,
        left: 48,
      },
      tooltip: {
        trigger: "axis",
        axisPointer: { type: "shadow" },
        backgroundColor: "#14253d",
        borderWidth: 0,
        textStyle: { color: "#fff" },
        formatter: (params) => {
          const point = Array.isArray(params)
            ? (params[0] as { name: string; value: number })
            : (params as { name: string; value: number });
          const percent =
            stats.total > 0
              ? ((point.value / stats.total) * 100).toFixed(1)
              : "0.0";
          return `<strong>${point.name}</strong><br/>${formatTokenValue(point.value, displayTokenUnit)} tokens · ${percent}%`;
        },
      },
      xAxis: {
        type: "category",
        data: stats.byModel.map((item) => item.name),
        axisTick: { alignWithLabel: true },
        axisLabel: {
          color: "#8692a3",
          fontSize: isRotated ? 10 : 11,
          interval: 0,
          rotate: isRotated ? 35 : 0,
          align: isRotated ? "right" : "center",
          verticalAlign: isRotated ? "middle" : "top",
          margin: isRotated ? 10 : 8,
          formatter: (value: string) =>
            value.length > 12 ? `${value.slice(0, 11)}…` : value,
        },
        axisLine: { lineStyle: { color: "#d8dee8" } },
      },
      yAxis: {
        type: "value",
        axisLabel: {
          color: "#8692a3",
          fontSize: 11,
          formatter: (value: number) =>
            formatTokenValue(value, displayTokenUnit, true),
        },
        splitLine: { lineStyle: { color: "#edf0f4" } },
      },
      series: [
        {
          type: "bar",
          barMaxWidth: 30,
          barCategoryGap: "25%",
          emphasis: {
            itemStyle: {
              shadowBlur: 8,
              shadowColor: "rgba(0, 0, 0, 0.15)",
            },
          },
          label: {
            show: true,
            position: "top",
            distance: 5,
            color: "#536579",
            fontSize: 9,
            fontWeight: 600,
            formatter: (params) => {
              const point = params as { value: number };
              return formatTokenValue(point.value, displayTokenUnit, true);
            },
          },
          labelLayout: { hideOverlap: true },
          data: stats.byModel.map((item) => ({
            value: item.tokens,
            itemStyle: { color: item.color, borderRadius: [4, 4, 0, 0] },
          })),
        },
      ],
    }),
    [stats.byModel, stats.total, displayTokenUnit, isRotated],
  );
  const donutOption = useMemo<echarts.EChartsOption>(
    () => ({
      tooltip: {
        trigger: "item",
        backgroundColor: "#14253d",
        borderWidth: 0,
        textStyle: { color: "#fff" },
        formatter: (params) => {
          const point = params as {
            name: string;
            value: number;
            percent: number;
          };
          return `<strong>${point.name}</strong><br/>${formatTokenValue(point.value, displayTokenUnit)} · ${point.percent.toFixed(1)}%`;
        },
      },
      series: [
        {
          type: "pie",
          radius: ["58%", "78%"],
          center: ["50%", "48%"],
          avoidLabelOverlap: true,
          label: { show: false },
          labelLine: { show: false },
          data: visibleShareModels.map((item) => ({
            name: item.name,
            value: item.tokens,
            itemStyle: { color: item.color },
          })),
        },
      ],
    }),
    [visibleShareModels, displayTokenUnit],
  );
  const toggleModel = (id: string) =>
    setSelectedModels(
      selectedModels.includes(id)
        ? selectedModels.filter((item) => item !== id)
        : [...selectedModels, id],
    );
  return (
    <div className="page-content">
      <div className="page-heading">
        <div>
          <div className="eyebrow">OVERVIEW / PERSONAL USAGE</div>
          <h1>Token 使用概览</h1>
          <p>掌握你的模型使用节奏，把每一次调用变成可读的数据。</p>
        </div>
        <div className="heading-actions">
          <button
            className="button secondary"
            onClick={onExport}
            disabled={exporting}
          >
            <Download size={16} />
            {exporting ? "导出中" : "导出 Excel"}
          </button>
          <button className="button primary" onClick={onNew}>
            <Plus size={17} />
            新增记录
          </button>
        </div>
      </div>
      <div className="toolbar">
        <div className="date-switcher">
          <button
            className="icon-button small"
            onClick={() => moveRange(-1)}
            aria-label="上一个时间段"
          >
            <ChevronLeft size={16} />
          </button>
          <CalendarDays size={16} />
          <span>
            {range.from === range.to ? (
              dateFormat(range.from)
            ) : (
              <>
                {dateFormat(range.from)} <i>至</i> {dateFormat(range.to)}
              </>
            )}
          </span>
          <button
            className="icon-button small"
            onClick={() => moveRange(1)}
            aria-label="下一个时间段"
          >
            <ChevronRight size={16} />
          </button>
        </div>
        <div className="preset-group">
          {datePresets.map((item) => (
            <button
              key={item.id}
              className={preset === item.id ? "active" : ""}
              onClick={() => {
                setCustomRangeOpen(false);
                changePreset(item.id);
              }}
            >
              {item.label}
            </button>
          ))}
          <button
            className={preset === "custom" ? "active" : ""}
            onClick={() => {
              setCustomRangeError("");
              setCustomRangeOpen((open) => !open);
            }}
          >
            自定义
          </button>
        </div>
        <div className="model-filter">
          <button
            className={`filter-trigger ${selectedModels.length ? "has-value" : ""}`}
            onClick={() => setModelMenu(!modelMenu)}
          >
            <Filter size={15} />
            {selectedModels.length
              ? `已选 ${selectedModels.length} 个模型`
              : "全部模型"}
            <ChevronDown size={14} />
          </button>
          {modelMenu && (
            <div className="dropdown-menu model-menu">
              <button
                onClick={() => setSelectedModels([])}
                className="dropdown-clear"
              >
                清除筛选
              </button>
              {models.map((model) => (
                <label key={model.id} className="model-option">
                  <input
                    type="checkbox"
                    checked={selectedModels.includes(model.id)}
                    onChange={() => toggleModel(model.id)}
                  />
                  <span
                    className="color-dot"
                    style={{ backgroundColor: model.color }}
                  />
                  {model.name}
                </label>
              ))}
            </div>
          )}
        </div>
      </div>
      {customRangeOpen && (
        <div className="custom-range-panel">
          <div
            className="custom-range-mode"
            role="group"
            aria-label="日期选择方式"
          >
            <button
              className={customMode === "day" ? "active" : ""}
              onClick={() => setCustomMode("day")}
            >
              单日
            </button>
            <button
              className={customMode === "range" ? "active" : ""}
              onClick={() => setCustomMode("range")}
            >
              日期区间
            </button>
          </div>
          <div className="custom-range-fields">
            {customMode === "day" ? (
              <label>
                选择日期
                <input
                  type="date"
                  value={customDate}
                  onChange={(event) => setCustomDate(event.target.value)}
                />
              </label>
            ) : (
              <>
                <label>
                  开始日期
                  <input
                    type="date"
                    value={customFrom}
                    max={customTo || undefined}
                    onChange={(event) => setCustomFrom(event.target.value)}
                  />
                </label>
                <span className="custom-range-separator">至</span>
                <label>
                  结束日期
                  <input
                    type="date"
                    value={customTo}
                    min={customFrom || undefined}
                    onChange={(event) => setCustomTo(event.target.value)}
                  />
                </label>
              </>
            )}
          </div>
          <div className="custom-range-actions">
            <span>
              {customRangeError ||
                (customMode === "day"
                  ? "仅查看所选日期的 Token 使用"
                  : "选择日期区间后，趋势图和统计指标会同步更新")}
            </span>
            <button className="button primary" onClick={submitCustomRange}>
              应用范围
            </button>
          </div>
        </div>
      )}
      <section className="metric-grid">
        <MetricCard
          label="总 Token 用量"
          value={formatTokenValue(stats.total, displayTokenUnit, true)}
          detail={`${formatTokenValue(stats.total, displayTokenUnit)} tokens`}
          accent="amber"
          icon={<Zap size={18} />}
        />
        <MetricCard
          label="日均使用量"
          value={formatTokenValue(stats.average, displayTokenUnit, true)}
          detail="按日历天数计算"
          accent="teal"
          icon={<BarChart3 size={18} />}
        />
        <MetricCard
          label="最高使用日"
          value={stats.peakDate ? formatDateLabel(stats.peakDate) : "—"}
          detail={
            stats.peakTokens
              ? `${formatTokenValue(stats.peakTokens, displayTokenUnit)} tokens`
              : "暂无数据"
          }
          accent="blue"
          icon={<CalendarDays size={18} />}
        />
        <MetricCard
          label="活跃模型"
          value={String(stats.activeModels)}
          detail="当前筛选范围内"
          accent="violet"
          icon={<Database size={18} />}
        />
      </section>
      <section className="chart-card trend-card">
        <div className="card-header">
          <div>
            <h2>每日使用趋势</h2>
            <span>按日汇总全部选中模型的 Token 用量</span>
          </div>
          <div className="chart-header-controls">
            <div className="legend-label">
              <span className="legend-line" />
              Token 数量
            </div>
            <label className="unit-control">
              <span>显示单位</span>
              <select
                aria-label="图表 Token 单位"
                value={displayTokenUnit}
                onChange={(event) =>
                  setDisplayTokenUnit(event.target.value as DisplayTokenUnit)
                }
              >
                <option value="auto">自动</option>
                <option value="1">1</option>
                <option value="M">M</option>
                <option value="B">B</option>
              </select>
            </label>
          </div>
        </div>
        {stats.total ? (
          <Chart option={trendOption} className="trend-chart" />
        ) : (
          <EmptyChart label="当前范围没有使用记录" />
        )}
      </section>
      <div className="chart-row">
        <section className="chart-card">
          <div className="card-header">
            <div>
              <h2>模型用量对比</h2>
              <span>累计 Token 使用量</span>
            </div>
            <button className="icon-button" aria-label="更多操作">
              <MoreHorizontal size={18} />
            </button>
          </div>
          {stats.total ? (
            <Chart option={barOption} className="bar-chart" />
          ) : (
            <EmptyChart label="暂无模型数据" />
          )}
        </section>
        <section className="chart-card model-share-card">
          <div className="card-header">
            <div>
              <h2>模型使用占比</h2>
              <span>当前范围内的 Token 分布</span>
            </div>
          </div>
          {stats.total ? (
            <div className="share-content">
              <Chart option={donutOption} className="donut-chart" />
              <div className="share-list">
                {visibleShareModels.map((item) => (
                  <div className="share-item" key={item.modelId}>
                    <div className="share-name">
                      <span
                        className="color-dot"
                        style={{ backgroundColor: item.color }}
                      />
                      <span>{item.name}</span>
                    </div>
                    <div className="share-values">
                      <strong>
                        {formatTokenValue(
                          item.tokens,
                          displayTokenUnit,
                          true,
                        )}
                      </strong>
                      <span>{formatPercent(item.share)}</span>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ) : (
            <EmptyChart label="暂无模型数据" />
          )}
        </section>
      </div>
      <section className="insight-strip">
        <div className="insight-icon">
          <Zap size={17} />
        </div>
        <div>
          <strong>
            {stats.peakDate
              ? `${formatDateLabel(stats.peakDate)} 是你的高峰日`
              : "开始记录你的 Token 使用"}
          </strong>
          <span>
            {stats.peakDate
              ? `当天使用 ${formatTokenValue(stats.peakTokens, displayTokenUnit)} tokens，占当前范围总量的 ${formatPercent(stats.total ? stats.peakTokens / stats.total : 0)}。`
              : "新增一条记录后，这里会显示你的使用节奏洞察。"}
          </span>
        </div>
        <button className="text-button" onClick={onNew}>
          记录今天 <ChevronRight size={15} />
        </button>
      </section>
    </div>
  );
}

const MetricCard = ({
  label,
  value,
  detail,
  accent,
  icon,
}: {
  label: string;
  value: string;
  detail: string;
  accent: string;
  icon: React.ReactNode;
}) => (
  <div className={`metric-card ${accent}`}>
    <div className="metric-top">
      <span>{label}</span>
      <span className="metric-icon">{icon}</span>
    </div>
    <strong>{value}</strong>
    <small>{detail}</small>
  </div>
);
const EmptyChart = ({ label }: { label: string }) => (
  <div className="empty-chart">
    <div className="empty-icon">
      <BarChart3 size={22} />
    </div>
    <strong>{label}</strong>
    <span>调整筛选范围或新增一条记录</span>
  </div>
);

function RecordsView({
  records,
  models,
  range,
  query,
  setQuery,
  onNew,
  onEdit,
  onDelete,
  onExport,
  exporting,
}: {
  records: UsageRecord[];
  models: Model[];
  range: DateRange;
  query: string;
  setQuery: (value: string) => void;
  onNew: () => void;
  onEdit: (record: UsageRecord) => void;
  onDelete: (record: UsageRecord) => void;
  onExport: () => void;
  exporting: boolean;
}) {
  return (
    <div className="page-content">
      <div className="page-heading compact">
        <div>
          <div className="eyebrow">DATA / DAILY LOG</div>
          <h1>使用记录</h1>
          <p>按日期整理每个模型的 Token 用量。</p>
        </div>
        <div className="heading-actions">
          <button
            className="button secondary"
            onClick={onExport}
            disabled={exporting}
          >
            <Download size={16} />
            {exporting ? "导出中" : "导出当前结果"}
          </button>
          <button className="button primary" onClick={onNew}>
            <Plus size={17} />
            新增记录
          </button>
        </div>
      </div>
      <div className="records-toolbar">
        <div className="search-box">
          <Search size={16} />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="搜索模型或备注"
          />
        </div>
        <div className="records-meta">
          <span>
            {range.from} 至 {range.to}
          </span>
          <span className="meta-divider" />
          <strong>{records.length} 条记录</strong>
        </div>
      </div>
      <div className="table-card">
        <table>
          <thead>
            <tr>
              <th>日期</th>
              <th>模型</th>
              <th>供应商</th>
              <th className="align-right">Token 数量</th>
              <th>备注</th>
              <th className="action-col" />
            </tr>
          </thead>
          <tbody>
            {records.map((record) => {
              const model = models.find((item) => item.id === record.modelId);
              return (
                <tr key={record.id}>
                  <td>
                    <strong>{record.date}</strong>
                  </td>
                  <td>
                    <span className="model-cell">
                      <span
                        className="color-dot"
                        style={{ backgroundColor: model?.color }}
                      />
                      {model?.name ?? "未知模型"}
                    </span>
                  </td>
                  <td>
                    <span className="provider-pill">
                      {model?.provider ?? "—"}
                    </span>
                  </td>
                  <td className="align-right token-cell">
                    {formatNumber(record.tokens)}
                  </td>
                  <td className="notes-cell">
                    {record.notes || <span className="muted-text">—</span>}
                  </td>
                  <td className="row-actions">
                    <button
                      className="icon-button small"
                      onClick={() => onEdit(record)}
                      aria-label="编辑记录"
                      title="编辑"
                    >
                      <Pencil size={15} />
                    </button>
                    <button
                      className="icon-button small danger"
                      onClick={() => onDelete(record)}
                      aria-label="删除记录"
                      title="删除"
                    >
                      <Trash2 size={15} />
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {records.length === 0 && (
          <div className="empty-table">
            <div className="empty-icon">
              <Database size={22} />
            </div>
            <strong>没有找到记录</strong>
            <span>尝试调整筛选条件，或新增一条使用记录。</span>
            <button className="button primary" onClick={onNew}>
              <Plus size={16} />
              新增记录
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

function ModelsView({
  models,
  records,
  onNew,
  onEdit,
  onToggle,
  onDelete,
}: {
  models: Model[];
  records: UsageRecord[];
  onNew: () => void;
  onEdit: (model: Model) => void;
  onToggle: (model: Model) => void;
  onDelete: (model: Model, recordCount: number) => void;
}) {
  return (
    <div className="page-content">
      <div className="page-heading compact">
        <div>
          <div className="eyebrow">CONFIG / MODEL CATALOG</div>
          <h1>模型管理</h1>
          <p>维护你的模型列表，停用不会影响历史记录。</p>
        </div>
        <button className="button primary" onClick={onNew}>
          <Plus size={17} />
          添加模型
        </button>
      </div>
      <div className="models-grid">
        {models.map((model) => {
          const modelRecords = records.filter(
            (record) => record.modelId === model.id,
          );
          const usage = modelRecords.reduce(
            (sum, record) => sum + record.tokens,
            0,
          );
          const hasHistory = modelRecords.length > 0;
          return (
            <article
              className={`model-card ${!model.active ? "inactive" : ""}`}
              key={model.id}
            >
              <div className="model-card-top">
                <span
                  className="model-swatch"
                  style={{ backgroundColor: model.color }}
                />
                <div className="model-title">
                  <h2>{model.name}</h2>
                  <span>{model.provider || "未设置供应商"}</span>
                </div>
                <button
                  className="icon-button small"
                  onClick={() => onEdit(model)}
                  aria-label="编辑模型"
                  title="编辑模型"
                >
                  <Pencil size={15} />
                </button>
                <button
                  className="icon-button small model-delete-button"
                  onClick={() => onDelete(model, modelRecords.length)}
                  aria-label="删除模型"
                  title={hasHistory ? "已有历史记录，只能停用" : "删除模型"}
                  disabled={hasHistory}
                >
                  <Trash2 size={15} />
                </button>
              </div>
              <div className="model-stats">
                <div>
                  <span>累计使用</span>
                  <strong>{formatCompact(usage)}</strong>
                </div>
                <div>
                  <span>记录数</span>
                  <strong>{modelRecords.length}</strong>
                </div>
              </div>
              <div className="model-card-footer">
                <span
                  className={`status-label ${model.active ? "active" : "inactive"}`}
                >
                  <span />
                  {model.active ? "使用中" : "已停用"}
                </span>
                <button className="text-button" onClick={() => onToggle(model)}>
                  {model.active ? "停用" : "启用"}
                </button>
              </div>
            </article>
          );
        })}
        <button className="add-model-card" onClick={onNew}>
          <Plus size={22} />
          <strong>添加模型</strong>
          <span>支持自定义名称、供应商和颜色</span>
        </button>
      </div>
    </div>
  );
}

function ExportView({
  range,
  setRange,
  stats,
  models,
  records,
  onBatchImport,
  onExport,
  onExportAll,
  exporting,
}: {
  range: DateRange;
  setRange: (range: DateRange) => void;
  stats: Stats;
  models: Model[];
  records: UsageRecord[];
  onBatchImport: (records: UsageRecord[], overwrite: boolean) => Promise<void>;
  onExport: () => void;
  onExportAll: () => void;
  exporting: boolean;
}) {
  const [batchImportOpen, setBatchImportOpen] = useState(false);
  return (
    <div className="page-content">
      <div className="page-heading compact">
        <div>
          <div className="eyebrow">INPUT / OUTPUT / WORKBOOK</div>
          <h1>导入导出</h1>
          <p>批量录入使用记录，或将本地统计整理成 Excel 文件。</p>
        </div>
        <div className="heading-actions">
          <button
            className="button secondary"
            onClick={() => setBatchImportOpen(true)}
          >
            <Upload size={16} />
            批量导入
          </button>
          <button
            className="button primary"
            onClick={onExport}
            disabled={exporting}
          >
            <Download size={17} />
            {exporting ? "正在生成" : "导出当前范围"}
          </button>
        </div>
      </div>
      <div className="export-layout">
        <section className="export-panel">
          <div className="export-panel-head">
            <div className="file-icon">
              <FileSpreadsheet size={24} />
            </div>
            <div>
              <h2>Token 使用统计</h2>
              <span>Excel 工作簿 · .xlsx</span>
            </div>
            <span className="ready-badge">可导出</span>
          </div>
          <div className="export-form">
            <label>
              统计开始日期
              <input
                type="date"
                value={range.from}
                onChange={(event) =>
                  setRange({ ...range, from: event.target.value })
                }
              />
            </label>
            <label>
              统计结束日期
              <input
                type="date"
                value={range.to}
                onChange={(event) =>
                  setRange({ ...range, to: event.target.value })
                }
              />
            </label>
          </div>
          <div className="export-includes">
            <h3>工作簿包含</h3>
            <div className="include-row">
              <span className="include-number">01</span>
              <div>
                <strong>使用明细</strong>
                <span>日期、模型、供应商、Token 数量和备注</span>
              </div>
            </div>
            <div className="include-row">
              <span className="include-number">02</span>
              <div>
                <strong>按日汇总</strong>
                <span>每天的 Token 总量和记录数量</span>
              </div>
            </div>
            <div className="include-row">
              <span className="include-number">03</span>
              <div>
                <strong>按模型汇总</strong>
                <span>模型用量、占比和记录天数</span>
              </div>
            </div>
          </div>
          <button
            className="button primary export-wide"
            onClick={onExport}
            disabled={exporting}
          >
            <Download size={17} />
            生成当前范围工作簿
          </button>
          <button
            className="button secondary export-wide"
            onClick={onExportAll}
            disabled={exporting}
          >
            <Download size={17} />
            生成全部数据工作簿
          </button>
        </section>
        <aside className="export-summary">
          <div className="summary-label">当前范围预览</div>
          <strong>{formatNumber(stats.total)}</strong>
          <span>总 Token</span>
          <div className="summary-divider" />
          <div className="summary-detail">
            <span>记录条数</span>
            <strong>
              {stats.daily.reduce(
                (sum, item) => sum + (item.tokens > 0 ? 1 : 0),
                0,
              )}{" "}
              天
            </strong>
          </div>
          <div className="summary-detail">
            <span>覆盖模型</span>
            <strong>{stats.activeModels} 个</strong>
          </div>
          <div className="summary-note">
            <Upload size={15} />
            数据不会上传到网络，文件只保存到你选择的位置。
          </div>
        </aside>
      </div>
      {batchImportOpen && (
        <BatchImportModal
          models={models.filter((model) => model.active)}
          records={records}
          onClose={() => setBatchImportOpen(false)}
          onImport={onBatchImport}
        />
      )}
    </div>
  );
}

function BatchImportModal({
  models,
  records,
  onClose,
  onImport,
}: {
  models: Model[];
  records: UsageRecord[];
  onClose: () => void;
  onImport: (records: UsageRecord[], overwrite: boolean) => Promise<void>;
}) {
  const [from, setFrom] = useState(todayString());
  const [to, setTo] = useState(todayString());
  const [modelId, setModelId] = useState(models[0]?.id ?? "");
  const [tokens, setTokens] = useState("");
  const [unit, setUnit] = useState<TokenUnit>("1");
  const [notes, setNotes] = useState("");
  const [overwrite, setOverwrite] = useState(false);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const dates = listDates(from, to);
  const tokenParts = tokens
    .split(/[\s,，]+/)
    .map((part) => part.trim())
    .filter(Boolean);
  const parsedValues: Array<number | null> = tokenParts.map((part) =>
    parseTokenInput(part, unit),
  );
  const dailyTokens: Array<number | null> =
    tokenParts.length === 1 && parsedValues[0] !== null
      ? dates.map(() => parsedValues[0])
      : tokenParts.length === dates.length
        ? parsedValues
        : [];
  const validDailyTokens =
    dailyTokens.length === dates.length &&
    dailyTokens.length > 0 &&
    dailyTokens.every((value): value is number => value !== null);
  const totalTokens = validDailyTokens
    ? dailyTokens.reduce((sum, value) => sum + value, 0)
    : null;
  const sameDailyTokens =
    validDailyTokens && dailyTokens.every((value) => value === dailyTokens[0]);
  const duplicateCount = dates.filter((date) =>
    records.some(
      (record) => record.date === date && record.modelId === modelId,
    ),
  ).length;
  const submit = async () => {
    if (
      !modelId ||
      !dates.length ||
      dates.length > 10_000 ||
      !validDailyTokens ||
      !totalTokens ||
      !Number.isSafeInteger(totalTokens)
    ) {
      setError(
        tokenParts.length > 1 && tokenParts.length !== dates.length
          ? "填写多个 Token 数量时，数量必须与日期天数一致"
          : "请选择模型和有效日期，并输入大于 0 的 Token 数量",
      );
      return;
    }
    if (duplicateCount > 0 && !overwrite) {
      setOverwrite(true);
      setError(`已有 ${duplicateCount} 条同日期同模型记录，再次点击将覆盖它们`);
      return;
    }
    const batch = dates.map((date, index) => ({
      id: makeId("batch-record"),
      date,
      modelId,
      tokens: dailyTokens[index] as number,
      notes: notes.trim(),
      createdAt: new Date().toISOString(),
    }));
    setSaving(true);
    try {
      await onImport(batch, overwrite);
      onClose();
    } catch (caught) {
      const appError = caught as { code?: string; message?: string };
      if (appError.code === "DUPLICATE_RECORD") {
        setOverwrite(true);
        setError("数据库中已有同日期同模型记录，再次点击将确认覆盖");
      } else setError(appError.message || "批量导入失败，请重试");
    } finally {
      setSaving(false);
    }
  };
  return (
    <Modal
      title="批量导入使用记录"
      onClose={saving ? () => undefined : onClose}
    >
      <div className="modal-form">
        <div className="batch-help">
          选择连续日期后，填一个 Token
          数量会应用到全部日期；也可以按日期逐行填写多个数量。
        </div>
        <label>
          使用模型
          <select
            value={modelId}
            onChange={(event) => setModelId(event.target.value)}
          >
            {models.length ? (
              models.map((model) => (
                <option key={model.id} value={model.id}>
                  {model.name} · {model.provider}
                </option>
              ))
            ) : (
              <option value="">请先添加并启用模型</option>
            )}
          </select>
        </label>
        <div className="export-form batch-date-form">
          <label>
            开始日期
            <input
              type="date"
              value={from}
              onChange={(event) => setFrom(event.target.value)}
            />
          </label>
          <label>
            结束日期
            <input
              type="date"
              value={to}
              onChange={(event) => setTo(event.target.value)}
            />
          </label>
        </div>
        <label>
          每日 Token 数量
          <div className="token-input-row">
            <textarea
              autoFocus
              inputMode="decimal"
              value={tokens}
              onChange={(event) => setTokens(event.target.value)}
              placeholder={
                unit === "1"
                  ? "例如 1250000"
                  : unit === "M"
                    ? "例如 1.5"
                    : "例如 0.2"
              }
              rows={2}
            />
            <select
              value={unit}
              onChange={(event) => setUnit(event.target.value as TokenUnit)}
            >
              <option value="1">1</option>
              <option value="M">M（百万）</option>
              <option value="B">B（十亿）</option>
            </select>
          </div>
        </label>
        <label>
          备注 <span className="optional">选填</span>
          <textarea
            value={notes}
            onChange={(event) => setNotes(event.target.value)}
            placeholder="应用到这批记录的备注"
            rows={2}
          />
        </label>
        {dates.length > 10_000 && (
          <div className="form-error">日期范围不能超过 10,000 天</div>
        )}
        {dates.length > 0 &&
          validDailyTokens &&
          totalTokens &&
          Number.isSafeInteger(totalTokens) && (
            <div className="batch-preview">
              <strong>将导入 {dates.length} 天</strong>
              <span>
                {sameDailyTokens
                  ? `每天 ${formatNumber(dailyTokens[0] as number)} tokens · `
                  : "按日期填写 · "}
                合计 {formatNumber(totalTokens)} tokens
              </span>
            </div>
          )}
        {error && <div className="form-error">{error}</div>}
        <div className="modal-actions">
          <button
            className="button secondary"
            onClick={onClose}
            disabled={saving}
          >
            取消
          </button>
          <button
            className="button primary"
            onClick={submit}
            disabled={saving || !models.length}
          >
            {saving ? "导入中" : overwrite ? "确认覆盖并导入" : "导入记录"}
          </button>
        </div>
      </div>
    </Modal>
  );
}

function RecordModal({
  record,
  models,
  onClose,
  onSave,
}: {
  record?: UsageRecord;
  models: Model[];
  onClose: () => void;
  onSave: (record: UsageRecord, overwrite: boolean) => boolean;
}) {
  const [date, setDate] = useState(record?.date ?? todayString());
  const [modelId, setModelId] = useState(
    record?.modelId ?? models[0]?.id ?? "",
  );
  const [tokens, setTokens] = useState(record ? String(record.tokens) : "");
  const [unit, setUnit] = useState<TokenUnit>("1");
  const [notes, setNotes] = useState(record?.notes ?? "");
  const [error, setError] = useState("");
  const [overwrite, setOverwrite] = useState(false);
  const submit = () => {
    const parsed = parseTokenInput(tokens, unit);
    if (!date || !modelId || !parsed) {
      setError("请选择日期、模型，并输入有效的 Token 数量");
      return;
    }
    const saved = onSave(
      {
        id: record?.id ?? makeId("record"),
        date,
        modelId,
        tokens: parsed,
        notes: notes.trim(),
        createdAt: record?.createdAt ?? new Date().toISOString(),
      },
      overwrite || Boolean(record),
    );
    if (!saved) {
      setOverwrite(true);
      setError("该日期已有此模型记录，请再次点击确认覆盖");
    }
  };
  return (
    <Modal title={record ? "编辑使用记录" : "新增使用记录"} onClose={onClose}>
      <div className="modal-form">
        <label>
          使用日期
          <input
            type="date"
            value={date}
            onChange={(event) => setDate(event.target.value)}
          />
        </label>
        <label>
          使用模型
          <select
            value={modelId}
            onChange={(event) => setModelId(event.target.value)}
          >
            {models.length ? (
              models.map((model) => (
                <option key={model.id} value={model.id}>
                  {model.name} · {model.provider}
                </option>
              ))
            ) : (
              <option value="">请先添加模型</option>
            )}
          </select>
        </label>
        <label>
          Token 数量
          <div className="token-input-row">
            <input
              autoFocus
              inputMode="decimal"
              value={tokens}
              onChange={(event) => setTokens(event.target.value)}
              placeholder={
                unit === "1"
                  ? "例如 1250000"
                  : unit === "M"
                    ? "例如 1.5"
                    : "例如 0.2"
              }
            />
            <select
              value={unit}
              onChange={(event) => setUnit(event.target.value as TokenUnit)}
            >
              <option value="1">1</option>
              <option value="M">M（百万）</option>
              <option value="B">B（十亿）</option>
            </select>
          </div>
        </label>
        <label>
          备注 <span className="optional">选填</span>
          <textarea
            value={notes}
            onChange={(event) => setNotes(event.target.value)}
            placeholder="记录这次使用的项目或场景"
            rows={3}
          />
        </label>
        {error && <div className="form-error">{error}</div>}
        <div className="modal-actions">
          <button className="button secondary" onClick={onClose}>
            取消
          </button>
          <button className="button primary" onClick={submit}>
            {overwrite ? "确认覆盖" : record ? "保存修改" : "保存记录"}
          </button>
        </div>
      </div>
    </Modal>
  );
}

function ModelModal({
  model,
  onClose,
  onSave,
}: {
  model?: Model;
  onClose: () => void;
  onSave: (model: Model) => void;
}) {
  const [name, setName] = useState(model?.name ?? "");
  const [provider, setProvider] = useState(model?.provider ?? "");
  const [color, setColor] = useState(model?.color ?? "#e5a84b");
  const [error, setError] = useState("");
  return (
    <Modal title={model ? "编辑模型" : "添加模型"} onClose={onClose}>
      <div className="modal-form">
        <label>
          模型名称
          <input
            autoFocus
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder="例如 GPT-5"
          />
        </label>
        <label>
          供应商 <span className="optional">选填</span>
          <input
            value={provider}
            onChange={(event) => setProvider(event.target.value)}
            placeholder="例如 OpenAI"
          />
        </label>
        <label>
          图表颜色
          <div className="color-input">
            <input
              type="color"
              value={color}
              onChange={(event) => setColor(event.target.value)}
            />
            <span>{color.toUpperCase()}</span>
          </div>
        </label>
        {error && <div className="form-error">{error}</div>}
        <div className="modal-actions">
          <button className="button secondary" onClick={onClose}>
            取消
          </button>
          <button
            className="button primary"
            onClick={() => {
              if (!name.trim()) {
                setError("请输入模型名称");
                return;
              }
              onSave({
                id: model?.id ?? makeId("model"),
                name: name.trim(),
                provider: provider.trim(),
                color,
                active: model?.active ?? true,
                createdAt: model?.createdAt ?? new Date().toISOString(),
              });
            }}
          >
            保存模型
          </button>
        </div>
      </div>
    </Modal>
  );
}

const extractErrorMessage = (err: unknown): string => {
  if (!err) return "检查更新失败，请确认网络连接";
  if (typeof err === "string") return err;
  if (typeof err === "object" && err !== null) {
    const obj = err as { message?: unknown; error?: unknown; code?: unknown };
    if (typeof obj.message === "string" && obj.message.trim()) return obj.message;
    if (typeof obj.error === "string" && obj.error.trim()) return obj.error;
    if (typeof obj.code === "string" && obj.code.trim()) return `更新检查异常 (${obj.code})`;
  }
  if (err instanceof Error && err.message) return err.message;
  return "检查更新出现异常，请稍后重试";
};

function UpdateModal({
  onClose,
  onToast,
}: {
  onClose: () => void;
  onToast: (message: string) => void;
}) {
  const [loading, setLoading] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [appInfo, setAppInfo] = useState<AppMetadata | null>(null);
  const [downloading, setDownloading] = useState(false);
  const [progress, setProgress] = useState<DownloadProgress | null>(null);
  const [backingUp, setBackingUp] = useState(false);
  const [updateResult, setUpdateResult] = useState<string | null>(null);

  const handleCheck = async () => {
    setLoading(true);
    setError(null);
    try {
      const info = await nativeCheckAppUpdate();
      setUpdateInfo(info);
    } catch (err) {
      setError(extractErrorMessage(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    nativeGetAppInfo().then(setAppInfo).catch(() => {});
    handleCheck();
  }, []);

  useEffect(() => {
    const unlisten = listenUpdateProgress((p) => {
      setProgress(p);
    });
    return () => {
      unlisten();
    };
  }, []);

  const handleBackupNow = async () => {
    setBackingUp(true);
    try {
      const res = await nativeBackupDatabaseNow();
      onToast(res.message);
      nativeGetAppInfo().then(setAppInfo).catch(() => {});
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      onToast(`备份失败: ${msg}`);
    } finally {
      setBackingUp(false);
    }
  };

  const handleStartUpdate = async () => {
    if (!updateInfo?.downloadUrl || !updateInfo?.assetName) {
      if (updateInfo?.releaseUrl) {
        window.open(updateInfo.releaseUrl, "_blank");
      }
      return;
    }
    setDownloading(true);
    setError(null);
    try {
      const res = await nativeDownloadAndInstallUpdate(
        updateInfo.downloadUrl,
        updateInfo.assetName,
      );
      setUpdateResult(res);
      onToast("安装包已就绪并拉起！您的本地数据已自动备份完好。");
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(`更新安装失败: ${msg}`);
    } finally {
      setDownloading(false);
    }
  };

  return (
    <Modal title="软件更新与数据管理" className="update-modal" onClose={onClose}>
      <div className="update-modal-body">
        <div className="update-header-info">
          <div>
            <div style={{ fontSize: 10, color: "var(--muted)", marginBottom: 4 }}>
              当前运行版本
            </div>
            <div className="version-badge">
              TokenScope v{appInfo?.version || "1.0.0"}
            </div>
          </div>
          <button
            className="button secondary"
            style={{ height: 32, padding: "0 10px", fontSize: 10 }}
            onClick={handleCheck}
            disabled={loading || downloading}
          >
            <RefreshCw size={13} className={loading ? "spin" : ""} />
            <span>{loading ? "正在检查..." : "检查更新"}</span>
          </button>
        </div>

        {error && (
          <div
            className="form-error"
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
            }}
          >
            <span>{error}</span>
            <button
              className="button secondary"
              style={{ height: 26, padding: "0 8px", fontSize: 9 }}
              onClick={handleCheck}
            >
              重试
            </button>
          </div>
        )}

        {loading && !updateInfo && (
          <div className="empty-chart" style={{ height: 110 }}>
            <div style={{ color: "var(--amber)" }}>
              <RefreshCw size={22} className="spin" />
            </div>
            <span>正在扫描本地工程文件与更新状态...</span>
          </div>
        )}

        {updateInfo && !updateInfo.hasUpdate && (
          <div className="update-status-card">
            <div className="update-tag-line">
              <span className="badge-latest">✓ 当前已是最新版</span>
              <span>{updateInfo.releaseName || "已与本地工程保持最新"}</span>
            </div>
            <div style={{ fontSize: 11, color: "var(--muted)", lineHeight: 1.5, whiteSpace: "pre-line" }}>
              {updateInfo.releaseNotes || "当前运行的程序已是最新构建版本，本地数据库与历史记录完好无损。"}
            </div>
            {updateInfo.isLocalUpdate && appInfo?.localProjectPath && (
              <div style={{ fontSize: 10, color: "var(--subtle)", marginTop: 6 }}>
                本地工程路径：{appInfo.localProjectPath}
              </div>
            )}
            {!updateInfo.isLocalUpdate && updateInfo.releaseUrl && (
              <div style={{ paddingTop: 4 }}>
                <a
                  href={updateInfo.releaseUrl}
                  target="_blank"
                  rel="noreferrer"
                  className="text-button"
                  style={{ display: "inline-flex", alignItems: "center", gap: 4 }}
                >
                  <span>访问 GitHub 仓库 Release 发布页</span>
                  <ExternalLink size={12} />
                </a>
              </div>
            )}
          </div>
        )}

        {updateInfo && updateInfo.hasUpdate && (
          <div className="update-status-card has-update">
            <div className="update-tag-line">
              <span className="badge-new">{updateInfo.isLocalUpdate ? "本地有更新" : "发现新版本"}</span>
              <span>
                {updateInfo.releaseName || updateInfo.latestVersion}
              </span>
            </div>

            {updateInfo.releaseDate && (
              <div style={{ fontSize: 10, color: "var(--subtle)" }}>
                构建/更新时间：{updateInfo.releaseDate}
              </div>
            )}

            {updateInfo.releaseNotes && (
              <div>
                <div style={{ fontSize: 10, color: "var(--muted)", marginBottom: 4 }}>
                  版本与工程状态：
                </div>
                <div className="update-notes-box" style={{ whiteSpace: "pre-line" }}>
                  {updateInfo.releaseNotes}
                </div>
              </div>
            )}

            <div className="data-safety-banner">
              <ShieldCheck size={20} style={{ flexShrink: 0, marginTop: 1 }} />
              <div>
                <strong>数据安全保证（不删除任何数据）</strong>
                <span>
                  本次升级采用无损更新机制，SQLite 历史数据存放在系统 AppData 目录，不受程序替换影响；启动更新前系统还将自动建立带时间戳的完整快照备份。
                </span>
              </div>
            </div>

            {downloading && progress && (
              <div className="update-progress-bar-wrap">
                <div className="update-progress-text">
                  <span>{updateInfo.isLocalUpdate ? "正在同步更新..." : "正在下载更新包..."}</span>
                  <span>{progress.percentage.toFixed(1)}%</span>
                </div>
                <div className="update-progress-bar">
                  <div
                    className="update-progress-fill"
                    style={{ width: `${progress.percentage}%` }}
                  />
                </div>
                <div className="update-progress-text" style={{ fontSize: 9 }}>
                  <span>已处理 {(progress.downloaded / 1024 / 1024).toFixed(2)} MB</span>
                  <span>
                    总计{" "}
                    {progress.total > 0
                      ? `${(progress.total / 1024 / 1024).toFixed(2)} MB`
                      : "完成"}
                  </span>
                </div>
              </div>
            )}

            {updateResult ? (
              <div
                style={{
                  padding: "10px 12px",
                  background: "#edf7f3",
                  border: "1px solid #ccebdb",
                  borderRadius: 6,
                  color: "#276749",
                  fontSize: 11,
                  lineHeight: 1.5,
                }}
              >
                {updateResult}
              </div>
            ) : (
              <div style={{ display: "flex", gap: 10, alignItems: "center", marginTop: 4 }}>
                {updateInfo.downloadUrl ? (
                  <button
                    className="button primary"
                    style={{ flex: 1, height: 38 }}
                    onClick={handleStartUpdate}
                    disabled={downloading}
                  >
                    <HardDriveDownload size={15} />
                    <span>
                      {downloading
                        ? "正在更新..."
                        : updateInfo.downloadUrl === "local://build_and_sync"
                        ? "一键本地编译并热更新（无损保留数据）"
                        : updateInfo.isLocalUpdate
                        ? "一键应用本地更新（无损保留数据）"
                        : "一键点击更新（无损保留数据）"}
                    </span>
                  </button>
                ) : (
                  <a
                    href={updateInfo.releaseUrl}
                    target="_blank"
                    rel="noreferrer"
                    className="button primary"
                    style={{ flex: 1, textDecoration: "none", height: 38 }}
                  >
                    <ExternalLink size={14} />
                    <span>前往发布页面查看</span>
                  </a>
                )}
                {!updateInfo.isLocalUpdate && updateInfo.downloadUrl && (
                  <a
                    href={updateInfo.releaseUrl}
                    target="_blank"
                    rel="noreferrer"
                    className="button secondary"
                    style={{ height: 38, textDecoration: "none" }}
                    title="在浏览器中查看 Release"
                  >
                    <ExternalLink size={14} />
                  </a>
                )}
              </div>
            )}
          </div>
        )}

        <div className="backup-section">
          <div>
            <strong>本地数据存储与安全</strong>
            <span>
              数据库位置：
              {appInfo?.databasePath
                ? appInfo.databasePath.split("\\").slice(-2).join("\\")
                : "AppData"}
              {appInfo?.backupCount ? ` · 已有 ${appInfo.backupCount} 个历史快照` : ""}
            </span>
          </div>
          <button
            className="button secondary"
            style={{ height: 30, padding: "0 10px", fontSize: 10 }}
            onClick={handleBackupNow}
            disabled={backingUp}
          >
            <span>{backingUp ? "备份中..." : "手动创建数据备份"}</span>
          </button>
        </div>
      </div>
    </Modal>
  );
}

const Modal = ({
  title,
  className = "",
  onClose,
  children,
}: {
  title: string;
  className?: string;
  onClose: () => void;
  children: React.ReactNode;
}) => (
  <div
    className="modal-backdrop"
    role="presentation"
    onMouseDown={(event) => {
      if (event.target === event.currentTarget) onClose();
    }}
  >
    <div className={`modal ${className}`} role="dialog" aria-modal="true" aria-label={title}>
      <div className="modal-header">
        <h2>{title}</h2>
        <button className="icon-button" onClick={onClose} aria-label="关闭">
          <X size={18} />
        </button>
      </div>
      {children}
    </div>
  </div>
);

export default App;
