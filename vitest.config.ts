import { configDefaults, defineConfig } from "vitest/config";

export default defineConfig({
  // plugins: [tsconfigPaths() as any],
  test: {
    // vitest >=4 no longer excludes dist/ by default; keep built test copies out of the run
    exclude: [...configDefaults.exclude, "**/dist/**"],
    dangerouslyIgnoreUnhandledErrors: true,
  },
});
