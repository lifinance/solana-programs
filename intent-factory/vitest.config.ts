import { loadEnv } from "vite"
import { defineConfig } from "vitest/config"

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "")

  return {
    test: {
      testTimeout: 60_000,
      hookTimeout: 30_000,
      include: ["tests/**/*.spec.ts"],
      globals: true,
      env: {
        JUPITER_API_KEY: env["JUPITER_API_KEY"] ?? "",
        SOLANA_RPC_URL: env["SOLANA_RPC_URL"] ?? "",
        JUPITER_SCENARIOS: env["JUPITER_SCENARIOS"] ?? "",
      },
    },
  }
})

