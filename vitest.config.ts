import { defineConfig } from "vitest/config";

export default defineConfig({
  // plugins: [tsconfigPaths() as any],
  test: {
    dangerouslyIgnoreUnhandledErrors: true,
  },
});
