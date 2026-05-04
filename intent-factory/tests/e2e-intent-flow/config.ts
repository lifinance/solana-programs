import { readFileSync } from "fs"
import {
  Connection,
  Keypair,
  PublicKey,
} from "@solana/web3.js"

// ---------------------------------------------------------------------------
// Environment configuration for the E2E intent flow runner
// ---------------------------------------------------------------------------

export interface E2EConfig {
  connection: Connection
  payer: Keypair

  programId: PublicKey
  recipient: PublicKey

  sourceMint: PublicKey
  destinationMint: PublicKey
  amountIn: bigint
  sourceDecimals: number
  minOut: bigint | null

  jupiterApiUrl: string
  jupiterApiKey: string
  jupiterSlippageBps: number
  jupiterMaxAccounts: number

  salt: Uint8Array

  jito: {
    blockEngineUrl: string
    tipPercentile: number
    tipLamportsFallback: bigint
    tipAccount: PublicKey
  }
}

const USDC_MINT = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
const USDT_MINT = "6p6xgHyF7AeE6TZkSmFsko444wqoP15icUSqi2jfGiPN"

// Well-known Jito tip accounts (mainnet)
const DEFAULT_JITO_TIP_ACCOUNT = "96gYZGLnJYVFmbjzopPSU6QiEV5fGqZNyN9nmNhvrZU5"
const DEFAULT_JITO_BLOCK_ENGINE = "https://mainnet.block-engine.jito.wtf/api/v1"

function requireEnv(key: string): string {
  const val = process.env[key]
  if (!val) throw new Error(`Missing required env var: ${key}`)
  return val
}

export function loadKeypair(pathOrJson: string): Keypair {
  let raw: string
  if (pathOrJson.startsWith("[") || pathOrJson.startsWith("{")) {
    raw = pathOrJson
  } else {
    raw = readFileSync(pathOrJson, "utf-8")
  }
  const parsed = JSON.parse(raw)
  const bytes = Array.isArray(parsed) ? parsed : parsed.secretKey
  return Keypair.fromSecretKey(new Uint8Array(bytes))
}

export function loadE2EConfig(): E2EConfig {
  const rpcUrl = requireEnv("SOLANA_RPC_URL")
  const connection = new Connection(rpcUrl, "confirmed")

  const payer = loadKeypair(requireEnv("PAYER_KEYPAIR"))
  const programId = new PublicKey(requireEnv("INTENT_FACTORY_PROGRAM_ID"))
  const recipient = new PublicKey(requireEnv("INTENT_RECIPIENT"))

  const sourceMint = new PublicKey(
    process.env["SOURCE_MINT"] ?? USDC_MINT,
  )
  const destinationMint = new PublicKey(
    process.env["DESTINATION_MINT"] ?? USDT_MINT,
  )
  const amountIn = BigInt(process.env["INTENT_AMOUNT_IN"] ?? "1000000")
  const sourceDecimals = Number(process.env["SOURCE_DECIMALS"] ?? "6")
  const minOutRaw = process.env["INTENT_MIN_OUT"]
  const minOut = minOutRaw ? BigInt(minOutRaw) : null

  const jupiterApiUrl = process.env["JUPITER_API_URL"] ?? "https://api.jup.ag"
  const jupiterApiKey = requireEnv("JUPITER_API_KEY")
  const jupiterSlippageBps = Number(process.env["JUPITER_SLIPPAGE_BPS"] ?? "50")
  const jupiterMaxAccounts = Number(process.env["JUPITER_MAX_ACCOUNTS"] ?? "64")

  const saltEnv = process.env["INTENT_SALT"]
  let salt: Uint8Array
  if (saltEnv) {
    const bytes = Buffer.from(saltEnv, "hex")
    if (bytes.length !== 32) throw new Error("INTENT_SALT must be 32 bytes hex")
    salt = new Uint8Array(bytes)
  } else {
    salt = new Uint8Array(32)
    crypto.getRandomValues(salt)
  }

  const jito = {
    blockEngineUrl:
      process.env["JITO_BLOCK_ENGINE_URL"] ?? DEFAULT_JITO_BLOCK_ENGINE,
    tipPercentile: Number(process.env["JITO_TIP_PERCENTILE"] ?? "75"),
    tipLamportsFallback: BigInt(process.env["JITO_TIP_LAMPORTS"] ?? "10000"),
    tipAccount: new PublicKey(
      process.env["JITO_TIP_ACCOUNT"] ?? DEFAULT_JITO_TIP_ACCOUNT,
    ),
  }

  return {
    connection,
    payer,
    programId,
    recipient,
    sourceMint,
    destinationMint,
    amountIn,
    sourceDecimals,
    minOut,
    jupiterApiUrl,
    jupiterApiKey,
    jupiterSlippageBps,
    jupiterMaxAccounts,
    salt,
    jito,
  }
}
