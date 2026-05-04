import {
  PublicKey,
  TransactionInstruction,
} from "@solana/web3.js"
import { getAssociatedTokenAddressSync } from "@solana/spl-token"

import {
  FixedSlot,
  convertToSymbolicIx,
} from "../../ts-client/src/index.js"
import type { SymbolicInstruction } from "../../ts-client/src/index.js"

// ---------------------------------------------------------------------------
// Jupiter API response shapes (mirrors the swap-instructions endpoint)
// ---------------------------------------------------------------------------

export interface JupiterAccountMeta {
  pubkey: string
  isSigner: boolean
  isWritable: boolean
}

export interface JupiterInstruction {
  programId: string
  accounts: JupiterAccountMeta[]
  data: string
}

export interface JupiterSwapInstructionsResponse {
  computeBudgetInstructions: JupiterInstruction[]
  setupInstructions: JupiterInstruction[]
  swapInstruction: JupiterInstruction
  cleanupInstruction?: JupiterInstruction | null
  otherInstructions?: JupiterInstruction[]
  tokenLedgerInstruction?: JupiterInstruction | null
  addressLookupTableAddresses: string[]
}

export interface JupiterQuoteResponse {
  inputMint: string
  inAmount: string
  outputMint: string
  outAmount: string
  otherAmountThreshold: string
  slippageBps: number
  routePlan: unknown[]
  [key: string]: unknown
}

// ---------------------------------------------------------------------------
// Partitioned instruction groups ready for the E2E flow
// ---------------------------------------------------------------------------

export interface PartitionedJupiterIxs {
  innerCpiIxs: SymbolicInstruction[]
  setupIxs: TransactionInstruction[]
  cleanupIxs: TransactionInstruction[]
  computeBudgetIxs: TransactionInstruction[]
  jupiterAltAddresses: string[]
}

// ---------------------------------------------------------------------------
// Jupiter API client
// ---------------------------------------------------------------------------

export async function fetchJupiterQuote(
  apiUrl: string,
  apiKey: string,
  inputMint: string,
  outputMint: string,
  amount: string,
  slippageBps: number,
  maxAccounts: number,
): Promise<JupiterQuoteResponse> {
  const params = new URLSearchParams({
    inputMint,
    outputMint,
    amount,
    slippageBps: String(slippageBps),
    maxAccounts: String(maxAccounts),
  })

  const res = await fetch(`${apiUrl}/swap/v1/quote?${params}`, {
    headers: { "x-api-key": apiKey },
  })

  if (!res.ok) {
    throw new Error(`Jupiter quote failed: ${res.status} ${await res.text()}`)
  }

  return res.json() as Promise<JupiterQuoteResponse>
}

export async function fetchJupiterSwapInstructions(
  apiUrl: string,
  apiKey: string,
  userPublicKey: string,
  payer: string,
  destinationTokenAccount: string,
  quote: JupiterQuoteResponse,
): Promise<JupiterSwapInstructionsResponse> {
  const res = await fetch(`${apiUrl}/swap/v1/swap-instructions`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "x-api-key": apiKey,
    },
    body: JSON.stringify({
      userPublicKey,
      payer,
      destinationTokenAccount,
      quoteResponse: quote,
      dynamicComputeUnitLimit: true,
      prioritizationFeeLamports: { autoMultiplier: 2 },
    }),
  })

  if (!res.ok) {
    throw new Error(
      `Jupiter swap-instructions failed: ${res.status} ${await res.text()}`,
    )
  }

  return res.json() as Promise<JupiterSwapInstructionsResponse>
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

function jupiterIxToTransactionIx(jix: JupiterInstruction): TransactionInstruction {
  return new TransactionInstruction({
    programId: new PublicKey(jix.programId),
    keys: jix.accounts.map((a) => ({
      pubkey: new PublicKey(a.pubkey),
      isSigner: a.isSigner,
      isWritable: a.isWritable,
    })),
    data: Buffer.from(jix.data, "base64"),
  })
}

function buildJupiterAccountMap(
  placeholderTaker: string,
  sourceAtaPubkey: string,
): Map<string, PublicKey> {
  return new Map([
    [placeholderTaker, FixedSlot.IntentPda],
    [sourceAtaPubkey, FixedSlot.SourceAta],
  ])
}

function jupiterInnerToSymbolicIx(
  jix: JupiterInstruction,
  accountMap: Map<string, PublicKey>,
): SymbolicInstruction {
  return convertToSymbolicIx(jupiterIxToTransactionIx(jix), {
    accountMap,
    signerPolicy: "intent-pda-only",
  })
}

// ---------------------------------------------------------------------------
// Partition Jupiter response into inner CPIs, setup IXs, and cleanup IXs
// ---------------------------------------------------------------------------

export function partitionJupiterIxs(
  instructions: JupiterSwapInstructionsResponse,
  intentPda: PublicKey,
  sourceMint: PublicKey,
): PartitionedJupiterIxs {
  if (instructions.tokenLedgerInstruction) {
    throw new Error(
      "Jupiter route requires a tokenLedgerInstruction which is not supported by this flow. " +
      "Request a route without token ledger (exact-input only).",
    )
  }

  const placeholderTaker = intentPda.toBase58()

  const sourceAta = getAssociatedTokenAddressSync(
    sourceMint,
    intentPda,
    true,
  ).toBase58()

  const accountMap = buildJupiterAccountMap(placeholderTaker, sourceAta)

  // Swap + otherInstructions are inner CPIs executed within execute_intent
  const innerCpiRaw: JupiterInstruction[] = [instructions.swapInstruction]
  if (instructions.otherInstructions) {
    innerCpiRaw.push(...instructions.otherInstructions)
  }
  const innerCpiIxs = innerCpiRaw.map((jix) =>
    jupiterInnerToSymbolicIx(jix, accountMap),
  )

  // Setup IXs execute before init/execute in their own tx
  const setupIxs = instructions.setupInstructions.map(jupiterIxToTransactionIx)

  // Cleanup IXs execute after execute_intent in their own tx
  const cleanupIxs: TransactionInstruction[] = []
  if (instructions.cleanupInstruction) {
    cleanupIxs.push(jupiterIxToTransactionIx(instructions.cleanupInstruction))
  }

  const computeBudgetIxs =
    instructions.computeBudgetInstructions.map(jupiterIxToTransactionIx)

  return {
    innerCpiIxs,
    setupIxs,
    cleanupIxs,
    computeBudgetIxs,
    jupiterAltAddresses: instructions.addressLookupTableAddresses,
  }
}
