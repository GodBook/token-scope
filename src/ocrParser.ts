import type { Model } from "./types";
import { todayString } from "./data";

export type OcrWord = {
  text: string;
  x: number;
  y: number;
  w: number;
  h: number;
};

export type OcrLine = {
  text: string;
  words?: OcrWord[];
};

export type ParsedOcrRecord = {
  id: string;
  selected: boolean;
  date: string;
  modelName: string;
  matchedModelId: string | null;
  isNewModel: boolean;
  suggestedProvider: string;
  tokens: number;
  cost?: string;
  notes: string;
};

/**
 * 常见供应商启发式推断
 */
export function inferProvider(modelName: string): string {
  const lower = modelName.toLowerCase();
  if (/^(gpt|o1|o3|o4|chatgpt|dall[-_]?e|whisper|text-embedding|davinci)/i.test(lower)) {
    return "OpenAI";
  }
  if (/^claude/i.test(lower)) {
    return "Anthropic";
  }
  if (/^(gemini|gemma|palm)/i.test(lower)) {
    return "Google";
  }
  if (/^deepseek/i.test(lower)) {
    return "DeepSeek";
  }
  if (/^(qwen|tongyi|wanx)/i.test(lower)) {
    return "Alibaba Cloud";
  }
  if (/^(llama|meta)/i.test(lower)) {
    return "Meta";
  }
  if (/^(mistral|codestral|mixtral|pixtral)/i.test(lower)) {
    return "Mistral AI";
  }
  if (/^(glm|chatglm|cogview)/i.test(lower)) {
    return "Zhipu AI";
  }
  if (/^(kimi|moonshot)/i.test(lower)) {
    return "Moonshot AI";
  }
  if (/^minimax/i.test(lower)) {
    return "MiniMax";
  }
  if (/^(yi-|lingyi)/i.test(lower)) {
    return "01.AI";
  }
  return "自定义";
}

const TABLE_HEADER_KEYWORDS = new Set([
  "model",
  "models",
  "modelname",
  "model_name",
  "prompt",
  "prompts",
  "prompttokens",
  "prompt_tokens",
  "completion",
  "completions",
  "completiontokens",
  "completion_tokens",
  "input",
  "output",
  "inputtokens",
  "outputtokens",
  "total",
  "totals",
  "totaltokens",
  "total_tokens",
  "tokens",
  "token",
  "cost",
  "price",
  "amount",
  "fee",
  "usage",
  "date",
  "time",
  "created_at",
  "status",
  "action",
  "actions",
  "operation",
  "模型",
  "模型名称",
  "模型名",
  "输入",
  "输出",
  "提示词",
  "补全",
  "总计",
  "合计",
  "用量",
  "总用量",
  "花费",
  "费用",
  "金额",
  "日期",
  "时间",
  "状态",
  "操作",
]);

/**
 * 判断是否为控制台表格表头或说明性干扰文本
 */
export function isTableHeaderText(text: string): boolean {
  if (!text) return false;
  const normalized = text.toLowerCase().replace(/[^a-z0-9\u4e00-\u9fa5]/g, "");
  return TABLE_HEADER_KEYWORDS.has(normalized);
}

/**
 * 清洗模型名称，去除 OCR 误识别的 logo 符号与杂散空格
 */
export function cleanModelName(raw: string): string {
  if (!raw) return "";
  let s = raw.trim();

  // 如果本身就是表头关键字，直接舍弃
  if (isTableHeaderText(s)) return "";

  // 去除开头的非字母数字的 logo 杂散字符 (如 + * ® © • ~ ^ 0 # & | \ 等及 markdown 列表符)
  s = s.replace(/^[-*•+®©~^0#&|/\\\[\]\d.]+\s*/, "");

  // 修复连接符、小数点周围的 OCR 空白 (如 "gemini -3 .8-flash" -> "gemini-3.8-flash")
  s = s.replace(/\s*([_.\-/:])\s*/g, "$1");

  // 修复常见的 OCR 单字符混淆错误 (如 f1ash -> flash, pr0 -> pro)
  s = s.replace(/f1ash/gi, "flash");
  s = s.replace(/fl4sh/gi, "flash");
  s = s.replace(/s0nnet/gi, "sonnet");
  s = s.replace(/ha1ku/gi, "haiku");
  s = s.replace(/pr0\b/gi, "pro");

  // 去除尾部标点符号
  s = s.replace(/[,;:]+$/, "");

  const trimmed = s.trim();
  if (isTableHeaderText(trimmed)) return "";
  return trimmed;
}

/**
 * 清洗并解析 Token 计数值
 */
export function cleanTokenCount(raw: string): number | null {
  if (!raw) return null;
  let s = raw.trim();

  // 如果明显是货币价格 (如 $39.72, ¥10.5, USD 12)，则不是 Token 计数
  if (/^[$¥￥€£]/.test(s) || /^(usd|cny|eur|gbp)\b/i.test(s)) return null;

  // 检查是否带有单位乘数 (如 25.1M, 1.5m, 500k, 2B, 125.4k tokens)
  const unitMatch = s.match(
    /^([\d.,\s]+)\s*([kKmMbB千百十万亿])\b(?:\s*tokens?|\s*tks?|\s*toks?)?$/i
  );
  if (unitMatch && unitMatch[2]) {
    const numClean = unitMatch[1].replace(/[\s,]/g, "");
    const num = parseFloat(numClean);
    if (!isNaN(num) && num > 0) {
      const u = unitMatch[2].toLowerCase();
      if (u === "k" || u === "千") return Math.round(num * 1_000);
      if (u === "m" || u === "百万") return Math.round(num * 1_000_000);
      if (u === "万") return Math.round(num * 10_000);
      if (u === "b" || u === "十亿") return Math.round(num * 1_000_000_000);
      if (u === "亿") return Math.round(num * 100_000_000);
    }
  }

  // 修复斜杠零 (slashed zero 0̸) 和 OCR 字母混淆
  // 当 e, E, o, O, θ, Q 紧邻数字或逗号时将其还原为 0
  s = s.replace(/([0-9,])\s*[eEoOθQ]+\s*([0-9,])/g, (match, p1, p2) => {
    const zeros = match.replace(/[^eEoOθQ]/g, "").length;
    return p1 + "0".repeat(zeros) + p2;
  });
  s = s.replace(/([0-9,])\s*[eEoOθQ]+/g, (match, p1) => {
    const zeros = match.replace(/[^eEoOθQ]/g, "").length;
    return p1 + "0".repeat(zeros);
  });
  s = s.replace(/[eEoOθQ]+\s*([0-9,])/g, (match, p1) => {
    const zeros = match.replace(/[^eEoOθQ]/g, "").length;
    return "0".repeat(zeros) + p1;
  });

  // 去除可能跟随的 "tokens" / "token"
  s = s.replace(/\s*(?:tokens?|tks?|toks?)$/i, "");

  // 去除所有的空格和逗号
  const digitsOnly = s.replace(/[\s,]/g, "");

  // 检查纯整数
  if (/^\d+$/.test(digitsOnly)) {
    const val = parseInt(digitsOnly, 10);
    if (val > 0 && Number.isSafeInteger(val)) {
      return val;
    }
  }

  return null;
}

/**
 * 提取货币金额费用
 */
export function extractCost(raw: string): string | null {
  if (!raw) return null;
  const match = raw.match(
    /([$¥￥€£]\s*\d+(?:\.\d{1,4})?|\b\d+(?:\.\d{1,4})?\s*[$¥￥€£]|\b(?:usd|cny|eur|gbp)\s*\d+(?:\.\d{1,4})?|\b\d+\.\d{2,4}\b)/i
  );
  if (match) {
    return match[0].replace(/\s+/g, "");
  }
  return null;
}

/**
 * 规范化模型名以进行模糊匹配
 */
function normalizeForMatching(name: string): string {
  return name.toLowerCase().replace(/[^a-z0-9]/g, "");
}

/**
 * 主解析入口：将 OCR 识别结果转换为结构化的用量待录入记录
 */
export function parseOcrResults(
  lines: OcrLine[],
  existingModels: Model[],
  defaultDate: string = todayString()
): ParsedOcrRecord[] {
  if (!lines || lines.length === 0) return [];

  // 判断是否拥有有效的空间包围盒数据
  const hasCoordinates = lines.some(
    (l) => l.words && l.words.length > 0 && typeof l.words[0].y === "number"
  );

  let rows: Array<{ items: OcrLine[]; avgY: number }> = [];

  if (hasCoordinates) {
    // 按 Y 轴坐标从上到下排序
    const validLines = lines.filter((l) => l.text && l.text.trim().length > 0);
    const sorted = [...validLines].sort((a, b) => {
      const ya = a.words?.[0]?.y ?? 0;
      const yb = b.words?.[0]?.y ?? 0;
      return ya - yb;
    });

    // 计算相邻行之间的聚类阈值
    const BAND_THRESHOLD = 42;

    for (const item of sorted) {
      const y = item.words?.[0]?.y ?? 0;
      let placed = false;
      for (const row of rows) {
        if (Math.abs(row.avgY - y) < BAND_THRESHOLD) {
          row.items.push(item);
          row.avgY = (row.avgY * (row.items.length - 1) + y) / row.items.length;
          placed = true;
          break;
        }
      }
      if (!placed) {
        rows.push({ avgY: y, items: [item] });
      }
    }
  } else {
    // 纯文本行降级处理：按文本顺序
    let currentRow: OcrLine[] = [];
    for (const line of lines) {
      const trimmed = line.text.trim();
      if (!trimmed) continue;
      currentRow.push(line);
      if (cleanTokenCount(trimmed) !== null && currentRow.length >= 2) {
        rows.push({ items: [...currentRow], avgY: 0 });
        currentRow = [];
      }
    }
    if (currentRow.length > 0) {
      rows.push({ items: currentRow, avgY: 0 });
    }
  }

  const results: ParsedOcrRecord[] = [];
  let recordIndex = 1;

  for (const row of rows) {
    // 检查本行是否全部由表头/说明性关键词组成
    const isPureHeaderRow = row.items.every((it) => {
      const t = it.text.trim();
      return !t || isTableHeaderText(t);
    });
    if (isPureHeaderRow) continue;

    let modelCandidate = "";
    let tokenCandidate: number | null = null;
    let costCandidate = "";
    const rawTokens: string[] = [];

    for (const item of row.items) {
      const text = item.text.trim();
      if (!text) continue;

      const cost = extractCost(text);
      if (cost && !costCandidate) {
        costCandidate = cost;
      }

      const tokens = cleanTokenCount(text);
      if (tokens !== null) {
        if (tokenCandidate === null || tokens > tokenCandidate) {
          tokenCandidate = tokens;
        }
      } else {
        const cleaned = cleanModelName(text);
        if (cleaned && !cleaned.startsWith("$") && !cleaned.startsWith("¥")) {
          if (!modelCandidate) {
            modelCandidate = cleaned;
          } else if (cleaned.length > modelCandidate.length) {
            modelCandidate = cleaned;
          }
        }
      }
      rawTokens.push(text);
    }

    // 如果未识别出有效模型名，且该行包含表头关键词，则跳过
    const hasHeaderTokens = row.items.some((it) => isTableHeaderText(it.text.trim()));
    if (!modelCandidate && hasHeaderTokens) {
      continue;
    }

    if (tokenCandidate === null) {
      for (const text of rawTokens) {
        const numbers = text.match(/\b\d[\d,\s]{2,}\b/g);
        if (numbers) {
          for (const n of numbers) {
            const parsed = cleanTokenCount(n);
            if (parsed !== null && (tokenCandidate === null || parsed > tokenCandidate)) {
              tokenCandidate = parsed;
            }
          }
        }
      }
    }

    if (modelCandidate || (tokenCandidate !== null && tokenCandidate > 0)) {
      const finalModelName = modelCandidate || `未命名模型-${recordIndex}`;
      const normalizedFinalName = normalizeForMatching(finalModelName);

      const matched = existingModels.find((m) => {
        const norm = normalizeForMatching(m.name);
        return norm === normalizedFinalName || norm.includes(normalizedFinalName) || normalizedFinalName.includes(norm);
      });

      const isNew = !matched;
      const matchedId = matched ? matched.id : null;
      const displayModelName = matched ? matched.name : finalModelName;
      const suggestedProvider = matched ? matched.provider : inferProvider(finalModelName);

      results.push({
        id: `ocr-${Date.now()}-${recordIndex++}`,
        selected: true,
        date: defaultDate,
        modelName: displayModelName,
        matchedModelId: matchedId,
        isNewModel: isNew,
        suggestedProvider,
        tokens: tokenCandidate ?? 0,
        cost: costCandidate,
        notes: costCandidate ? `费用: ${costCandidate}` : "",
      });
    }
  }

  return results;
}
