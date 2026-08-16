import { defineConfig } from "vitest/config";

export default defineConfig({
  define: {
    __APP_VERSION__: JSON.stringify("test")
  },
  test: {
    environment: "jsdom",
    globals: true,
    include: ["test/**/*.test.js"],
    setupFiles: ["./test/setup.js"],
    clearMocks: true,
    restoreMocks: true,
    coverage: {
      provider: "v8",
      reporter: ["text", "html", "lcov"],
      reportsDirectory: "./coverage",
      all: true,
      include: ["src/**/*.js"],
      exclude: ["test/**", "**/*.test.js", "build.js", "scripts/**"],
    },
  },
});
