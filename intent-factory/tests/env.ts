export interface TestEnv {
  jupiterApiKey: string
  jupiterApiUrl: string
  solanaRpcUrl: string
  jupiterScenarios: string[]
}

export function loadTestEnv(): TestEnv {
  const raw = process.env["JUPITER_SCENARIOS"] ?? ""
  const jupiterScenarios = raw
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean)

  return {
    jupiterApiKey: process.env["JUPITER_API_KEY"] ?? "",
    jupiterApiUrl:
      process.env["JUPITER_API_URL"] ?? "https://api.jup.ag",
    solanaRpcUrl: process.env["SOLANA_RPC_URL"] ?? "",
    jupiterScenarios,
  }
}

export function requireJupiterApiKey(env: TestEnv): string {
  if (!env.jupiterApiKey) {
    throw new Error(
      "JUPITER_API_KEY is required for Jupiter measurement tests. " +
        "Set it via .env or JUPITER_API_KEY=... npx vitest run tests/tx_size_jupiter.spec.ts"
    )
  }
  return env.jupiterApiKey
}

export function requireSolanaRpcUrl(env: TestEnv): string {
  if (!env.solanaRpcUrl) {
    throw new Error(
      "SOLANA_RPC_URL is required for Jupiter measurement tests (ALT loading). " +
        "Set it via .env or SOLANA_RPC_URL=... npx vitest run tests/tx_size_jupiter.spec.ts"
    )
  }
  return env.solanaRpcUrl
}
