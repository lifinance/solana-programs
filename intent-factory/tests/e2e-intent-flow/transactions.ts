import {
  Connection,
  Keypair,
  PublicKey,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
  AddressLookupTableAccount,
} from "@solana/web3.js"

import { log, logError } from "./log.js"

const COMPUTE_BUDGET_PROGRAM_ID = new PublicKey(
  "ComputeBudget111111111111111111111111111111",
)
const SET_COMPUTE_UNIT_PRICE_DISC = 0x03

// ---------------------------------------------------------------------------
// Build, sign, simulate, and send a v0 transaction
// ---------------------------------------------------------------------------

export async function buildV0Tx(
  connection: Connection,
  payer: PublicKey,
  instructions: TransactionInstruction[],
  signers: Keypair[],
  lookupTables?: AddressLookupTableAccount[],
): Promise<VersionedTransaction> {
  const { blockhash } = await connection.getLatestBlockhash("confirmed")

  const message = new TransactionMessage({
    payerKey: payer,
    recentBlockhash: blockhash,
    instructions,
  }).compileToV0Message(lookupTables)

  const tx = new VersionedTransaction(message)
  tx.sign(signers)
  return tx
}

export async function simulateAndSend(
  connection: Connection,
  tx: VersionedTransaction,
  label: string,
): Promise<string> {
  const sim = await connection.simulateTransaction(tx, { commitment: "confirmed" })
  if (sim.value.err) {
    logError(`[${label}] simulation failed:`, sim.value.err)
    if (sim.value.logs) logError(sim.value.logs.join("\n"))
    throw new Error(`Simulation failed for ${label}`)
  }
  log(`[${label}] simulation OK — CU used: ${sim.value.unitsConsumed ?? "n/a"}`)


//// BREAKPOINT
log("BREAKPOINT 1")
process.exit(1)

  const sig = await connection.sendTransaction(tx, { skipPreflight: true })
  log(`[${label}] sent: ${sig}`)

  const confirmation = await connection.confirmTransaction(sig, "confirmed")
  if (confirmation.value.err) {
    throw new Error(`[${label}] confirmation failed: ${JSON.stringify(confirmation.value.err)}`)
  }
  log(`[${label}] confirmed`)
  return sig
}

// ---------------------------------------------------------------------------
// Load address lookup tables from chain
// ---------------------------------------------------------------------------

const altCache = new Map<string, AddressLookupTableAccount>()

export async function loadALTs(
  connection: Connection,
  addresses: string[],
): Promise<AddressLookupTableAccount[]> {
  const loaded: AddressLookupTableAccount[] = []

  for (const addr of addresses) {
    const cached = altCache.get(addr)
    if (cached) {
      loaded.push(cached)
      continue
    }

    const result = await connection.getAddressLookupTable(new PublicKey(addr))
    if (!result.value) {
      throw new Error(`Failed to load ALT ${addr}: account not found on-chain`)
    }

    altCache.set(addr, result.value)
    loaded.push(result.value)
  }

  return loaded
}

// ---------------------------------------------------------------------------
// Strip SetComputeUnitPrice IXs (for Jito bundle mode)
// ---------------------------------------------------------------------------

export function stripCuPrice(
  ixs: TransactionInstruction[],
): TransactionInstruction[] {
  return ixs.filter((ix) => {
    if (!ix.programId.equals(COMPUTE_BUDGET_PROGRAM_ID)) return true
    if (ix.data.length === 0) return true
    return ix.data[0] !== SET_COMPUTE_UNIT_PRICE_DISC
  })
}
