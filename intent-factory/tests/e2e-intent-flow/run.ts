import {
  PublicKey,
  TransactionInstruction,
  AddressLookupTableAccount,
} from "@solana/web3.js"
import {
  getAssociatedTokenAddressSync,
  createAssociatedTokenAccountIdempotentInstruction,
  createTransferCheckedInstruction,
} from "@solana/spl-token"

import {
  buildInitIntentIx,
  buildExecuteIntentIx,
  deriveSourceAta,
} from "../../ts-client/src/index.js"
import type { IntentHeader } from "../../ts-client/src/index.js"

import { loadE2EConfig } from "./config.js"
import {
  fetchJupiterQuote,
  fetchJupiterSwapInstructions,
  partitionJupiterIxs,
} from "./jupiter.js"
import type { PartitionedJupiterIxs } from "./jupiter.js"
import {
  buildV0Tx,
  simulateAndSend,
  loadALTs,
  stripCuPrice,
} from "./transactions.js"
import {
  buildTipTx,
  fetchJitoTipLamports,
  simulateBundle,
  sendBundle,
  getBundleStatus,
} from "./jito.js"
import { getAnalysisPath, log, logError } from "./log.js"

// ===========================================================================
// Step 1 — Load environment configuration
// ===========================================================================

const cfg = loadE2EConfig()
const { connection, payer, programId } = cfg

log("=== Intent Factory E2E Flow ===")
log(`  Analysis:     ${getAnalysisPath()}`)
log(`  RPC:          ${connection.rpcEndpoint}`)
log(`  Payer:        ${payer.publicKey.toBase58()}`)
log(`  Program:      ${programId.toBase58()}`)
log(`  Recipient:    ${cfg.recipient.toBase58()}`)
log(`  Source mint:  ${cfg.sourceMint.toBase58()}`)
log(`  Dest mint:    ${cfg.destinationMint.toBase58()}`)
log(`  Amount in:    ${cfg.amountIn}`)
log()

// ===========================================================================
// Step 2 — Fetch Jupiter quote to determine minimum output before header
// construction, since outcome amounts are part of the canonical header bytes
// and therefore affect the PDA derivation.
// ===========================================================================

log("[jupiter] Fetching quote...")
const quote = await fetchJupiterQuote(
  cfg.jupiterApiUrl,
  cfg.jupiterApiKey,
  cfg.sourceMint.toBase58(),
  cfg.destinationMint.toBase58(),
  String(cfg.amountIn),
  cfg.jupiterSlippageBps,
  cfg.jupiterMaxAccounts,
)
log(`[jupiter] Quote: ${quote.inAmount} ${quote.inputMint} -> ${quote.outAmount} ${quote.outputMint}`)

const minOut = cfg.minOut ?? BigInt(quote.otherAmountThreshold)
log(`[jupiter] Min out: ${minOut} (otherAmountThreshold; outAmount: ${quote.outAmount})`)
log()

// ===========================================================================
// Step 3 — Construct the IntentHeader with final params (including minOut)
// ===========================================================================

// Recipient destination ATA — used as the outcome account in the header
const recipientDestAta = getAssociatedTokenAddressSync(
  cfg.destinationMint,
  cfg.recipient,
  true,
)

const header: IntentHeader = {
  user: payer.publicKey,
  srcMint: cfg.sourceMint,
  amountIn: cfg.amountIn,
  outcomes: [
    {
      mint: cfg.destinationMint,
      account: recipientDestAta,
      amount: minOut,
    },
  ],
  deadline: BigInt(Math.floor(Date.now() / 1000) + 3600),
  salt: cfg.salt,
  executor: payer.publicKey,
}

// ===========================================================================
// Step 4 — Derive intent PDA and canonical source ATA from final header
// ===========================================================================

const { initIx, intentPda, bump } = buildInitIntentIx({
  header,
  payer: payer.publicKey,
  programId,
})

const sourceAta = deriveSourceAta(intentPda, cfg.sourceMint)

log(`  Intent PDA:   ${intentPda.toBase58()} (bump ${bump})`)
log(`  Source ATA:   ${sourceAta.toBase58()}`)
log(`  Dest ATA:     ${recipientDestAta.toBase58()}`)
log()

// ===========================================================================
// Step 5 — Build funding setup IXs: create PDA source ATA + transfer funds
// ===========================================================================

const payerSourceAta = getAssociatedTokenAddressSync(
  cfg.sourceMint,
  payer.publicKey,
)

// 5a. Create the PDA's source mint ATA (idempotent — safe to re-run)
const createSourceAtaIx = createAssociatedTokenAccountIdempotentInstruction(
  payer.publicKey,
  sourceAta,
  intentPda,
  cfg.sourceMint,
)

// 5b. Transfer exactly amountIn from payer -> PDA source ATA
const fundSourceAtaIx = createTransferCheckedInstruction(
  payerSourceAta,
  cfg.sourceMint,
  sourceAta,
  payer.publicKey,
  Number(cfg.amountIn),
  cfg.sourceDecimals,
)

// 5c. Create the recipient's destination ATA (idempotent)
const createDestAtaIx = createAssociatedTokenAccountIdempotentInstruction(
  payer.publicKey,
  recipientDestAta,
  cfg.recipient,
  cfg.destinationMint,
)

// ===========================================================================
// Step 6 — Fetch Jupiter swap instructions. PDA is the token authority
// (userPublicKey), executor wallet pays for setup/rent (payer), and output
// is routed to the recipient's destination ATA (destinationTokenAccount).
// ===========================================================================

log("[jupiter] Fetching swap instructions...")
const swapInstructions = await fetchJupiterSwapInstructions(
  cfg.jupiterApiUrl,
  cfg.jupiterApiKey,
  intentPda.toBase58(),
  payer.publicKey.toBase58(),
  recipientDestAta.toBase58(),
  quote,
)

// ===========================================================================
// Step 7 — Partition Jupiter IXs: inner CPIs, setup IXs, cleanup IXs
// ===========================================================================

const partitioned: PartitionedJupiterIxs = partitionJupiterIxs(
  swapInstructions,
  intentPda,
  cfg.sourceMint,
)

log(`[jupiter] Inner CPIs: ${partitioned.innerCpiIxs.length}`)
log(`[jupiter] Setup IXs: ${partitioned.setupIxs.length}`)
log(`[jupiter] Cleanup IXs: ${partitioned.cleanupIxs.length}`)
log(`[jupiter] Compute budget IXs: ${partitioned.computeBudgetIxs.length}`)
log(`[jupiter] ALT addresses: ${partitioned.jupiterAltAddresses.length}`)

// Validate that no setup/cleanup IX requires the intent PDA as signer —
// these execute outside execute_intent and cannot access PDA authority
assertPeripheralSignersAreSafe(partitioned.setupIxs, [payer.publicKey])
assertPeripheralSignersAreSafe(partitioned.cleanupIxs, [payer.publicKey])

// ===========================================================================
// Step 8 — Build execute_intent IX with encoded Jupiter symbolic IXs
// ===========================================================================

const { executeIx } = buildExecuteIntentIx({
  header,
  symbolicIxs: partitioned.innerCpiIxs,
  programId,
})

// ===========================================================================
// Step 9 — Load Jupiter ALTs from chain (shared by both execution paths)
// ===========================================================================

let jupiterALTs: AddressLookupTableAccount[] = []
if (partitioned.jupiterAltAddresses.length > 0) {
  log("[alt] Loading Jupiter ALTs...")
  jupiterALTs = await loadALTs(connection, partitioned.jupiterAltAddresses)
  log(`[alt] Loaded ${jupiterALTs.length} ALTs`)
}

// ===========================================================================
// Step 10 — Execute: select normal or Jito bundle path
// ===========================================================================

const mode = (process.env["EXECUTION_MODE"] ?? "normal").toLowerCase()

if (mode === "normal") {
  await executeNormalFlow(partitioned, jupiterALTs)
} else if (mode === "jito") {
  await executeJitoFlow(partitioned, jupiterALTs)
} else {
  logError(`Unknown EXECUTION_MODE: ${mode}. Use "normal" or "jito".`)
  process.exit(1)
}

// ===========================================================================
// Normal execution path — setup tx, init tx, execute tx, cleanup tx
// ===========================================================================

async function executeNormalFlow(
  part: PartitionedJupiterIxs,
  alts: AddressLookupTableAccount[],
) {
  log("\n=== Normal Execution Path ===\n")

  // TX 1 — Setup: create source ATA, fund it, create dest ATA, Jupiter setup IXs
  const setupIxs: TransactionInstruction[] = [
    createSourceAtaIx,
    fundSourceAtaIx,
    createDestAtaIx,
    ...part.setupIxs,
  ]

  log(`[setup-tx] Building with ${setupIxs.length} IXs...`)
  const setupTx = await buildV0Tx(
    connection, payer.publicKey, setupIxs, [payer], alts,
  )
  await simulateAndSend(connection, setupTx, "setup-tx")

//// BREAKPOINT
log("BREAKPOINT 0")
process.exit(0)

  // TX 2 — Init: store the intent on-chain via init_intent
  log("[init-tx] Building...")
  const initTx = await buildV0Tx(
    connection, payer.publicKey, [initIx], [payer],
  )
  await simulateAndSend(connection, initTx, "init-tx")

  // TX 3 — Execute: compute budget IXs + execute_intent
  const executeIxs: TransactionInstruction[] = [
    ...part.computeBudgetIxs,
    executeIx,
  ]

  log(`[execute-tx] Building with ${executeIxs.length} IXs...`)
  const executeTx = await buildV0Tx(
    connection, payer.publicKey, executeIxs, [payer], alts,
  )
  await simulateAndSend(connection, executeTx, "execute-tx")

  // TX 4 — Cleanup: Jupiter cleanup IXs in their own tx after execute
  if (part.cleanupIxs.length > 0) {
    log(`[cleanup-tx] Building with ${part.cleanupIxs.length} IXs...`)
    const cleanupTx = await buildV0Tx(
      connection, payer.publicKey, part.cleanupIxs, [payer], alts,
    )
    await simulateAndSend(connection, cleanupTx, "cleanup-tx")
  }

  await printResults()
}

// ===========================================================================
// Jito execution path — pre-bundle setup tx, then tip + init + execute as a
// three-tx Jito bundle (CU price stripped), then cleanup tx.
// ===========================================================================

async function executeJitoFlow(
  part: PartitionedJupiterIxs,
  alts: AddressLookupTableAccount[],
) {
  log("\n=== Jito Bundle Execution Path ===\n")

  const { jito } = cfg

  // Resolve tip lamports: dynamic from QuickNode tip-floor, fallback to config
  const tipLamports = await fetchJitoTipLamports(
    connection.rpcEndpoint,
    jito.tipPercentile,
    jito.tipLamportsFallback,
  )

  // Pre-bundle TX — create ATAs, fund, Jupiter setup IXs
  const preIxs: TransactionInstruction[] = [
    createSourceAtaIx,
    fundSourceAtaIx,
    createDestAtaIx,
    ...part.setupIxs,
  ]

  log(`[setup-tx] Building pre-bundle setup with ${preIxs.length} IXs...`)
  const setupTx = await buildV0Tx(
    connection, payer.publicKey, preIxs, [payer], alts,
  )
  await simulateAndSend(connection, setupTx, "setup-tx (pre-bundle)")

  // Bundle TX 1 — Jito tip (incentivizes bundle inclusion)
  log(`[bundle-tx-1] Building tip tx (${tipLamports} lamports)...`)
  const tipTx = await buildTipTx(connection, payer, jito.tipAccount, tipLamports)

  // Bundle TX 2 — init_intent
  log("[bundle-tx-2] Building init_intent...")
  const initBundleTx = await buildV0Tx(
    connection, payer.publicKey, [initIx], [payer],
  )

  // Bundle TX 3 — execute_intent (CU price IXs stripped for Jito)
  const strippedComputeIxs = stripCuPrice(part.computeBudgetIxs)
  const bundleExecuteIxs: TransactionInstruction[] = [
    ...strippedComputeIxs,
    executeIx,
  ]

  log(`[bundle-tx-3] Building execute_intent with ${bundleExecuteIxs.length} IXs...`)
  const executeBundleTx = await buildV0Tx(
    connection, payer.publicKey, bundleExecuteIxs, [payer], alts,
  )

  const bundleTxs = [tipTx, initBundleTx, executeBundleTx]

  // Simulate the full Jito bundle before submission
  log(`[jito] Simulating bundle (${bundleTxs.length} txs)...`)
  await simulateBundle(jito.blockEngineUrl, bundleTxs)

  // Submit the bundle after simulation succeeds
  log("[jito] Sending bundle...")
  const bundleId = await sendBundle(jito.blockEngineUrl, bundleTxs)

  // Poll bundle landing status
  log("[jito] Polling bundle status...")
  await sleep(2000)
  const status = await getBundleStatus(jito.blockEngineUrl, bundleId)
  log("[jito] Bundle status:", status)

  // Post-bundle TX — Jupiter cleanup IXs in their own tx after execute
  if (part.cleanupIxs.length > 0) {
    log(`[cleanup-tx] Building post-bundle cleanup with ${part.cleanupIxs.length} IXs...`)
    const cleanupTx = await buildV0Tx(
      connection, payer.publicKey, part.cleanupIxs, [payer], alts,
    )
    await simulateAndSend(connection, cleanupTx, "cleanup-tx (post-bundle)")
  }

  await printResults()
}

// ===========================================================================
// Post-execution: print derived addresses and token balances
// ===========================================================================

async function printResults() {
  log("\n=== Results ===")
  log(`  Intent PDA:     ${intentPda.toBase58()}`)
  log(`  Source ATA:     ${sourceAta.toBase58()}`)
  log(`  Dest ATA:       ${recipientDestAta.toBase58()}`)

  try {
    const destBalance = await connection.getTokenAccountBalance(recipientDestAta)
    log(`  Dest balance:   ${destBalance.value.uiAmountString ?? "(n/a)"}`)
  } catch {
    log("  Dest balance:   (account not found or not yet reflected)")
  }

  try {
    const srcBalance = await connection.getTokenAccountBalance(sourceAta)
    log(`  Source balance:  ${srcBalance.value.uiAmountString ?? "0"} (should be 0 after execute)`)
  } catch {
    log("  Source balance:  (account not found)")
  }

  log("\nDone.")
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

// ---------------------------------------------------------------------------
// Validate that peripheral IXs do not require PDA signing
// ---------------------------------------------------------------------------

function assertPeripheralSignersAreSafe(
  peripheralIxs: TransactionInstruction[],
  allowedSigners: PublicKey[],
): void {
  const allowed = new Set(allowedSigners.map((pk) => pk.toBase58()))

  for (const ix of peripheralIxs) {
    for (const key of ix.keys) {
      if (key.isSigner && !allowed.has(key.pubkey.toBase58())) {
        throw new Error(
          `Jupiter peripheral IX requires signer ${key.pubkey.toBase58()} ` +
          `which is not payer or executor. Route cannot be prepared without PDA signing.`,
        )
      }
    }
  }
}
