import { describe, expect, it } from "vitest";
import {
  cleanModelName,
  cleanTokenCount,
  extractCost,
  inferProvider,
  parseOcrResults,
  type OcrLine,
} from "./ocrParser";
import type { Model } from "./types";

describe("ocrParser helper functions", () => {
  it("cleanModelName cleans artifacts and spacing correctly", () => {
    expect(cleanModelName("gpt-6-astra")).toBe("gpt-6-astra");
    expect(cleanModelName("+ gemini -3 .8-flash")).toBe("gemini-3.8-flash");
    expect(cleanModelName("® claude-3-5-s0nnet")).toBe("claude-3-5-sonnet");
    expect(cleanModelName("* deepseek-r1")).toBe("deepseek-r1");
    expect(cleanModelName("gemini-3.8-f1ash")).toBe("gemini-3.8-flash");
  });

  it("cleanTokenCount parses integers, handles commas and slashed zero ee/oo", () => {
    expect(cleanTokenCount("25,163,163")).toBe(25163163);
    // Slashed zero '0̸' OCR confusions
    expect(cleanTokenCount("18, 7ee,129")).toBe(18700129);
    expect(cleanTokenCount("18,7ee,129")).toBe(18700129);
    expect(cleanTokenCount("1,5oo,000")).toBe(1500000);

    // Multipliers
    expect(cleanTokenCount("25.1M")).toBe(25100000);
    expect(cleanTokenCount("500k")).toBe(500000);
    expect(cleanTokenCount("1.2B")).toBe(1200000000);

    // Rejects currency
    expect(cleanTokenCount("$39.72")).toBeNull();
    expect(cleanTokenCount("$1.0998")).toBeNull();
  });

  it("extractCost detects currency amounts", () => {
    expect(extractCost("$39.72")).toBe("$39.72");
    expect(extractCost("$1.0998")).toBe("$1.0998");
    expect(extractCost("¥ 128.50")).toBe("¥128.50");
  });

  it("inferProvider infers common providers accurately", () => {
    expect(inferProvider("gpt-6-astra")).toBe("OpenAI");
    expect(inferProvider("gemini-3.8-flash")).toBe("Google");
    expect(inferProvider("claude-sonnet-4")).toBe("Anthropic");
    expect(inferProvider("deepseek-v3")).toBe("DeepSeek");
    expect(inferProvider("qwen-2.5-max")).toBe("Alibaba Cloud");
  });
});

describe("parseOcrResults with user uploaded sample image data", () => {
  const sampleOcrData: OcrLine[] = [
    {
      text: "25,163,163",
      words: [{ y: 24, w: 113, h: 17, text: "25,163,163", x: 530 }],
    },
    {
      text: "gpt-6-astra",
      words: [{ y: 36, w: 125, h: 17, text: "gpt-6-astra", x: 34 }],
    },
    {
      text: "$39.72",
      words: [{ y: 48, w: 56, h: 15, text: "$39.72", x: 587 }],
    },
    {
      text: "18, 7ee,129",
      words: [
        { y: 128, w: 30, h: 17, text: "18,", x: 530 },
        { y: 128, w: 79, h: 17, text: "7ee,129", x: 564 },
      ],
    },
    {
      text: "gemini -3 .8-flash",
      words: [
        { y: 138, w: 67, h: 19, text: "gemini", x: 34 },
        { y: 140, w: 19, h: 13, text: "-3", x: 105 },
        { y: 139, w: 88, h: 14, text: ".8-flash", x: 129 },
      ],
    },
    {
      text: "$1.0998",
      words: [{ y: 153, w: 66, h: 15, text: "$1.0998", x: 578 }],
    },
  ];

  const existingModels: Model[] = [
    {
      id: "gemini-flash",
      name: "gemini-3.8-flash",
      provider: "Google",
      color: "#5bb9a4",
      active: true,
      createdAt: "2026-01-01",
    },
  ];

  it("accurately extracts both rows matching user request", () => {
    const today = "2026-09-12";
    const records = parseOcrResults(sampleOcrData, existingModels, today);

    expect(records.length).toBe(2);

    // Row 1: gpt-6-astra
    expect(records[0].modelName).toBe("gpt-6-astra");
    expect(records[0].tokens).toBe(25163163);
    expect(records[0].date).toBe(today);
    expect(records[0].notes).toBe("费用: $39.72");
    expect(records[0].isNewModel).toBe(true);
    expect(records[0].suggestedProvider).toBe("OpenAI");

    // Row 2: gemini-3.8-flash
    expect(records[1].modelName).toBe("gemini-3.8-flash");
    expect(records[1].tokens).toBe(18700129);
    expect(records[1].date).toBe(today);
    expect(records[1].notes).toBe("费用: $1.0998");
    expect(records[1].isNewModel).toBe(false);
    expect(records[1].matchedModelId).toBe("gemini-flash");
  });

  it("filters out table header rows and does not generate false records", () => {
    const tableWithHeader: OcrLine[] = [
      // Table Header Row
      {
        text: "Model Name",
        words: [{ y: 10, w: 90, h: 16, text: "Model Name", x: 20 }],
      },
      {
        text: "Total Tokens",
        words: [{ y: 10, w: 90, h: 16, text: "Total Tokens", x: 200 }],
      },
      {
        text: "Cost",
        words: [{ y: 10, w: 40, h: 16, text: "Cost", x: 350 }],
      },
      // Data Row
      {
        text: "claude-3-5-sonnet-20241022",
        words: [{ y: 60, w: 200, h: 16, text: "claude-3-5-sonnet-20241022", x: 20 }],
      },
      {
        text: "125.4k tokens",
        words: [{ y: 60, w: 100, h: 16, text: "125.4k tokens", x: 200 }],
      },
      {
        text: "$ 12.50",
        words: [{ y: 60, w: 60, h: 16, text: "$ 12.50", x: 350 }],
      },
    ];

    const records = parseOcrResults(tableWithHeader, existingModels, "2026-09-12");
    expect(records.length).toBe(1);
    expect(records[0].modelName).toBe("claude-3-5-sonnet-20241022");
    expect(records[0].tokens).toBe(125400);
    expect(records[0].notes).toBe("费用: $12.50");
    expect(records[0].suggestedProvider).toBe("Anthropic");
  });
});

