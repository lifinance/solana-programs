import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  TransactionInstruction,
  VersionedTransaction,
} from "@solana/web3.js"

import { log, logError } from "./log.js"
import { buildV0Tx } from "./transactions.js"

// ---------------------------------------------------------------------------
// Jito bundle helpers — direct block-engine JSON-RPC, no SDK dependency
// ---------------------------------------------------------------------------

interface JitoRpcResponse<T> {
  jsonrpc: "2.0"
  id: number
  result?: T
  error?: { code: number; message: string }
}

async function jitoRpc<T>(
  blockEngineUrl: string,
  method: string,
  params: unknown[],
): Promise<T> {
  const res = await fetch(blockEngineUrl, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: 1,
      method,
      params,
    }),
  })

  if (!res.ok) {
    throw new Error(`Jito RPC HTTP error: ${res.status} ${await res.text()}`)
  }

  const body = (await res.json()) as JitoRpcResponse<T>
  if (body.error) {
    throw new Error(`Jito RPC error: ${body.error.code} — ${body.error.message}`)
  }

  return body.result as T
}

// ---------------------------------------------------------------------------
// QuickNode getTipFloor — dynamic Jito tip resolution
// ---------------------------------------------------------------------------

const MAX_JITO_TIP_LAMPORTS = 0.003 * 1_000_000_000

interface TipFloorEntry {
  landed_tips_25th_percentile: number
  landed_tips_50th_percentile: number
  landed_tips_75th_percentile: number
  landed_tips_95th_percentile: number
  landed_tips_99th_percentile: number
  ema_landed_tips_50th_percentile: number
}

type TipFloorPercentileKey = keyof Omit<TipFloorEntry, "ema_landed_tips_50th_percentile">

/**
 * Fetches the Jito tip floor from QuickNode's `getTipFloor` RPC method,
 * selects the requested percentile, converts SOL to lamports, and caps
 * at MAX_JITO_TIP_LAMPORTS (0.003 SOL).
 *
 * Falls back to `fallbackLamports` if the RPC call fails.
 */
export async function fetchJitoTipLamports(
  rpcUrl: string,
  percentile: number,
  fallbackLamports: bigint,
): Promise<bigint> {
  const statsKey = `landed_tips_${percentile}th_percentile` as TipFloorPercentileKey

  try {
    const res = await fetch(rpcUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "getTipFloor",
        params: [],
      }),
    })

    if (!res.ok) {
      throw new Error(`getTipFloor HTTP error: ${res.status}`)
    }

    const body = (await res.json()) as { result?: TipFloorEntry[] }
    const entry = body.result?.[0]

    if (!entry || !(statsKey in entry)) {
      throw new Error(
        `getTipFloor: percentile ${percentile} not found in response`,
      )
    }

    const solValue = entry[statsKey]
    const rawLamports = Math.floor(solValue * 1_000_000_000)
    const capped = Math.min(rawLamports, MAX_JITO_TIP_LAMPORTS)
    const tipLamports = BigInt(capped)

    log(`[jito-tip] Percentile: ${percentile}th`)
    log(`[jito-tip] Raw SOL value: ${solValue}`)
    log(`[jito-tip] Computed lamports: ${rawLamports}`)
    log(`[jito-tip] Cap (0.003 SOL): ${MAX_JITO_TIP_LAMPORTS}`)
    log(`[jito-tip] Final tip: ${tipLamports} lamports`)

    return tipLamports
  } catch (err) {
    logError(`[jito-tip] Failed to fetch tip floor, using fallback ${fallbackLamports}:`, err)
    return fallbackLamports
  }
}

// ---------------------------------------------------------------------------
// Build the Jito tip instruction
// ---------------------------------------------------------------------------

export function buildTipIx(
  payer: PublicKey,
  tipAccount: PublicKey,
  tipLamports: bigint,
): TransactionInstruction {
  return SystemProgram.transfer({
    fromPubkey: payer,
    toPubkey: tipAccount,
    lamports: tipLamports,
  })
}

export async function buildTipTx(
  connection: Connection,
  payer: Keypair,
  tipAccount: PublicKey,
  tipLamports: bigint,
): Promise<VersionedTransaction> {
  const tipIx = buildTipIx(payer.publicKey, tipAccount, tipLamports)
  return buildV0Tx(connection, payer.publicKey, [tipIx], [payer])
}

// ---------------------------------------------------------------------------
// Simulate + submit a bundle of serialized transactions
// ---------------------------------------------------------------------------

function serializeTxs(txs: VersionedTransaction[]): string[] {
  return txs.map((tx) =>
    Buffer.from(tx.serialize()).toString("base64"),
  )
}

export async function simulateBundle(
  blockEngineUrl: string,
  txs: VersionedTransaction[],
): Promise<void> {
  const encoded = serializeTxs(txs)

  const result = await jitoRpc<{
    context: { slot: number }
    value: { summary: string; transactionResults: unknown[] }
  }>(blockEngineUrl, "simulateBundle", [
    { encodedTransactions: encoded },
    { simulationBank: "processed", skipSigVerify: true },
  ])

  const value =
    typeof result === "object" && result !== null
      ? (result as Record<string, unknown>).value ?? result
      : result

  log("[jito] simulateBundle result:", value)

  assertBundleSimulationOk(value)
}

function assertBundleSimulationOk(value: unknown): void {
  if (typeof value !== "object" || value === null) return

  const obj = value as Record<string, unknown>

  const summary = obj.summary
  if (typeof summary === "string") {
    const lower = summary.toLowerCase()
    if (lower !== "succeeded" && lower !== "success") {
      throw new Error(
        `Jito bundle simulation failed (summary: "${summary}"): ${JSON.stringify(value, null, 2)}`,
      )
    }
  }

  const txResults = obj.transactionResults
  if (!Array.isArray(txResults)) return

  for (let i = 0; i < txResults.length; i++) {
    const entry = txResults[i] as Record<string, unknown> | null
    if (typeof entry !== "object" || entry === null) continue

    const err = entry.err ?? entry.error
    if (err !== null && err !== undefined) {
      throw new Error(
        `Jito bundle simulation: tx[${i}] failed: ${JSON.stringify(err)}`,
      )
    }
  }
}

export async function sendBundle(
  blockEngineUrl: string,
  txs: VersionedTransaction[],
): Promise<string> {
  const encoded = serializeTxs(txs)

  const bundleId = await jitoRpc<string>(
    blockEngineUrl,
    "sendBundle",
    [encoded],
  )

  log(`[jito] bundle submitted: ${bundleId}`)
  return bundleId
}

export async function getBundleStatus(
  blockEngineUrl: string,
  bundleId: string,
): Promise<unknown> {
  return jitoRpc<unknown>(
    blockEngineUrl,
    "getBundleStatuses",
    [[bundleId]],
  )
}
