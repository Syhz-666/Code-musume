import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // M1：只出报告、不设硬阈值（DoD A16 的「core 纯函数 ≥80%」是 M2 的口径，避免两套标准）
    coverage: {
      provider: "v8",
      // `coverage.all` 已不存在于 vitest 5——把未被 import 的文件也纳入统计只能靠 include
      include: ["src/**/*.ts"],
      reporter: ["text", "json-summary"],
      reportsDirectory: "./coverage",
      reportOnFailure: true,
    },
  },
});
