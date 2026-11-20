import { describe, it, expect, afterAll } from "vitest"
import {
  PublicKey,
  SystemProgram,
  SYSVAR_CLOCK_PUBKEY,
  AddressLookupTableAccount,
} from "@solana/web3.js"
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token"
import { writeFileSync } from "fs"
import { resolve } from "path"

import {
  FixedSlot,
  buildExecuteIntentTx,
  buildRefundIntentTx,
  encodeIntentHeader,
  encodeCalls,
  dedupeAccounts,
  NAMED_PREFIX,
} from "../ts-client/src/index.js"
import type {
  IntentHeader,
  SymbolicInstruction,
} from "../ts-client/src/index.js"

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const SOLANA_TX_LIMIT = 1232
const SOLANA_TX_PRACTICAL = 1232

const PROGRAM_ID = PublicKey.unique()
const PAYER = PublicKey.unique()

// ---------------------------------------------------------------------------
// Scenario result type
// ---------------------------------------------------------------------------

interface ScenarioResult {
  name: string
  kind: "execute" | "refund"
  calls: number
  namedAccounts: number
  tailAccounts: number
  signatures: number
  headerBytes: number
  callsBytes: number
  messageBytes: number
  transactionBytes: number
  usesAlt: boolean
  altAddressCount: number
  estimatedCU: number | null
  fits: boolean
  classification: "fits" | "phase-2 pressure" | "blocker"
}

const results: ScenarioResult[] = []

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeHeader(overrides?: Partial<IntentHeader>): IntentHeader {
  return {
    user: PublicKey.unique(),
    srcMint: PublicKey.unique(),
    amountIn: 1_000_000n,
    outcomes: [
      {
        mint: PublicKey.unique(),
        account: PublicKey.unique(),
        amount: 950_000n,
      },
    ],
    deadline: 1_700_000_000n,
    salt: new Uint8Array(32).fill(0x07),
    executor: PublicKey.unique(),
    ...overrides,
  }
}

function makeSymIx(
  programId: PublicKey,
  keys: Array<{ pubkey: PublicKey; isSigner: boolean; isWritable: boolean }>,
  dataLen = 0
): SymbolicInstruction {
  return {
    programId,
    keys,
    data: new Uint8Array(dataLen).fill(0xaa),
  }
}

function makeCommonALT(): AddressLookupTableAccount {
  const altAddresses = [
    PROGRAM_ID,
    TOKEN_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,
    SystemProgram.programId,
    SYSVAR_CLOCK_PUBKEY,
  ]

  return new AddressLookupTableAccount({
    key: PublicKey.unique(),
    state: {
      deactivationSlot: BigInt("18446744073709551615"),
      lastExtendedSlot: 0,
      lastExtendedSlotStartIndex: 0,
      authority: PublicKey.unique(),
      addresses: altAddresses,
    },
  })
}

function classify(txBytes: number): "fits" | "phase-2 pressure" | "blocker" {
  if (txBytes <= SOLANA_TX_PRACTICAL * 0.85) return "fits"
  if (txBytes <= SOLANA_TX_PRACTICAL) return "phase-2 pressure"
  return "blocker"
}

function measureExecute(
  name: string,
  header: IntentHeader,
  symbolicIxs: SymbolicInstruction[],
  alt?: AddressLookupTableAccount,
  estimatedCU: number | null = null
): ScenarioResult {
  const result = buildExecuteIntentTx({
    header,
    symbolicIxs,
    payer: PAYER,
    programId: PROGRAM_ID,
    lookupTables: alt ? [alt] : undefined,
  })

  let txBytes: number
  let msgBytes: number
  try {
    txBytes = result.tx.serialize().length
    msgBytes = result.tx.message.serialize().length
  } catch {
    const sigCount = 2
    const staticKeyCount = result.tx.message.staticAccountKeys.length
    const ixDataLen = result.callsBytes.length + 8 + 4
    const accountKeyIndexCount = 4 + result.tailPubkeys.length
    msgBytes = 1 + 32 + 32 + staticKeyCount * 32 + 3 + 1 + accountKeyIndexCount + ixDataLen + 4
    txBytes = 1 + sigCount * 64 + msgBytes
  }

  const entry: ScenarioResult = {
    name,
    kind: "execute",
    calls: symbolicIxs.length,
    namedAccounts: 4,
    tailAccounts: result.tailPubkeys.length,
    signatures: 2,
    headerBytes: result.headerBytes.length,
    callsBytes: result.callsBytes.length,
    messageBytes: msgBytes,
    transactionBytes: txBytes,
    usesAlt: !!alt,
    altAddressCount: alt ? alt.state.addresses.length : 0,
    estimatedCU,
    fits: txBytes <= SOLANA_TX_PRACTICAL,
    classification: classify(txBytes),
  }

  results.push(entry)
  return entry
}

function measureRefund(
  name: string,
  header: IntentHeader,
  _symbolicIxs: SymbolicInstruction[],
  alt?: AddressLookupTableAccount,
  estimatedCU: number | null = null
): ScenarioResult {
  const headerBytes = encodeIntentHeader(header)

  const refundResult = buildRefundIntentTx({
    headerBytes,
    payer: PAYER,
    programId: PROGRAM_ID,
    lookupTables: alt ? [alt] : undefined,
  })

  const txBytes = refundResult.tx.serialize().length
  const msgBytes = refundResult.tx.message.serialize().length

  const entry: ScenarioResult = {
    name,
    kind: "refund",
    calls: 0,
    namedAccounts: 10,
    tailAccounts: 0,
    signatures: 1,
    headerBytes: headerBytes.length,
    callsBytes: 0,
    messageBytes: msgBytes,
    transactionBytes: txBytes,
    usesAlt: !!alt,
    altAddressCount: alt ? alt.state.addresses.length : 0,
    estimatedCU,
    fits: txBytes <= SOLANA_TX_PRACTICAL,
    classification: classify(txBytes),
  }

  results.push(entry)
  return entry
}

// ---------------------------------------------------------------------------
// Scenario fixtures
// ---------------------------------------------------------------------------

function minimalExecuteIxs(): {
  header: IntentHeader
  ixs: SymbolicInstruction[]
} {
  const tokenProgram = TOKEN_PROGRAM_ID
  const header = makeHeader()

  const ix1 = makeSymIx(
    tokenProgram,
    [
      { pubkey: FixedSlot.SourceAta, isSigner: false, isWritable: true },
      { pubkey: PublicKey.unique(), isSigner: false, isWritable: false },
      { pubkey: PublicKey.unique(), isSigner: false, isWritable: true },
      { pubkey: FixedSlot.IntentPda, isSigner: true, isWritable: false },
    ],
    9
  )

  const ix2 = makeSymIx(
    tokenProgram,
    [
      { pubkey: FixedSlot.SourceAta, isSigner: false, isWritable: true },
      { pubkey: PublicKey.unique(), isSigner: false, isWritable: false },
      { pubkey: PublicKey.unique(), isSigner: false, isWritable: true },
      { pubkey: FixedSlot.IntentPda, isSigner: true, isWritable: false },
    ],
    9
  )

  return { header, ixs: [ix1, ix2] }
}

function typicalExecuteIxs(): {
  header: IntentHeader
  ixs: SymbolicInstruction[]
} {
  const outcomeAccount1 = PublicKey.unique()
  const outcomeAccount2 = PublicKey.unique()

  const header = makeHeader({
    outcomes: [
      { mint: PublicKey.unique(), account: outcomeAccount1, amount: 950_000n },
      { mint: PublicKey.unique(), account: outcomeAccount2, amount: 5_000n },
    ],
    executor: PublicKey.unique(),
  })

  const tokenProgram = TOKEN_PROGRAM_ID
  const srcMintAcc = PublicKey.unique()

  const transferToOutcome1 = makeSymIx(
    tokenProgram,
    [
      { pubkey: FixedSlot.SourceAta, isSigner: false, isWritable: true },
      { pubkey: srcMintAcc, isSigner: false, isWritable: false },
      { pubkey: outcomeAccount1, isSigner: false, isWritable: true },
      { pubkey: FixedSlot.IntentPda, isSigner: true, isWritable: false },
    ],
    9
  )

  const transferToOutcome2 = makeSymIx(
    tokenProgram,
    [
      { pubkey: FixedSlot.SourceAta, isSigner: false, isWritable: true },
      { pubkey: srcMintAcc, isSigner: false, isWritable: false },
      { pubkey: outcomeAccount2, isSigner: false, isWritable: true },
      { pubkey: FixedSlot.IntentPda, isSigner: true, isWritable: false },
    ],
    9
  )

  return { header, ixs: [transferToOutcome1, transferToOutcome2] }
}

function worstCaseExecuteIxs(): {
  header: IntentHeader
  ixs: SymbolicInstruction[]
} {
  const header = makeHeader({
    outcomes: Array.from({ length: 4 }, (_, i) => ({
      mint: PublicKey.unique(),
      account: PublicKey.unique(),
      amount: BigInt((i + 1) * 1000),
    })),
    executor: PublicKey.unique(),
  })

  const ixs: SymbolicInstruction[] = []
  const programs = Array.from({ length: 4 }, () => PublicKey.unique())
  const extraAccounts = Array.from({ length: 12 }, () => PublicKey.unique())

  for (let c = 0; c < 8; c++) {
    const program = programs[c % programs.length]!
    const keys = [
      { pubkey: FixedSlot.IntentPda, isSigner: true, isWritable: true },
      { pubkey: FixedSlot.SourceAta, isSigner: false, isWritable: true },
    ]

    const numExtra = Math.min(c + 1, 4)
    for (let e = 0; e < numExtra; e++) {
      keys.push({
        pubkey: extraAccounts[(c * 2 + e) % extraAccounts.length]!,
        isSigner: false,
        isWritable: e % 2 === 0,
      })
    }

    ixs.push(makeSymIx(program, keys, Math.min((c + 1) * 16, 128)))
  }

  return { header, ixs }
}

// ---------------------------------------------------------------------------
// Test suites
// ---------------------------------------------------------------------------

describe("Tx Size Measurements", () => {
  const alt = makeCommonALT()

  describe("Scenario 1: Minimal execute", () => {
    it("measures without ALT", () => {
      const { header, ixs } = minimalExecuteIxs()
      const r = measureExecute("minimal_execute_no_alt", header, ixs, undefined, 25_000)
      expect(r.transactionBytes).toBeLessThanOrEqual(SOLANA_TX_LIMIT)
      expect(r.fits).toBe(true)
    })
  })

  describe("Scenario 2: Typical POC execute", () => {
    it("measures without ALT", () => {
      const { header, ixs } = typicalExecuteIxs()
      const r = measureExecute("typical_execute_no_alt", header, ixs, undefined, 50_000)
      expect(r.transactionBytes).toBeLessThanOrEqual(SOLANA_TX_LIMIT)
    })

    it("measures with common ALT", () => {
      const { header, ixs } = typicalExecuteIxs()
      const r = measureExecute("typical_execute_with_alt", header, ixs, alt, 50_000)
      expect(r.transactionBytes).toBeLessThanOrEqual(SOLANA_TX_LIMIT)
      expect(r.fits).toBe(true)
    })
  })

  describe("Scenario 3: Worst-case POC execute", () => {
    it("measures without ALT", () => {
      const { header, ixs } = worstCaseExecuteIxs()
      const r = measureExecute("worst_case_execute_no_alt", header, ixs, undefined, 200_000)
    })

    it("measures with common ALT", () => {
      const { header, ixs } = worstCaseExecuteIxs()
      const r = measureExecute("worst_case_execute_with_alt", header, ixs, alt, 200_000)
    })
  })

  describe("Scenario 4: Refund populated source", () => {
    it("measures without ALT", () => {
      const { header, ixs } = minimalExecuteIxs()
      const r = measureRefund("refund_populated_no_alt", header, ixs, undefined, 30_000)
      expect(r.transactionBytes).toBeLessThanOrEqual(SOLANA_TX_LIMIT)
    })

    it("measures with common ALT", () => {
      const { header, ixs } = minimalExecuteIxs()
      const r = measureRefund("refund_populated_with_alt", header, ixs, alt, 30_000)
      expect(r.transactionBytes).toBeLessThanOrEqual(SOLANA_TX_LIMIT)
      expect(r.fits).toBe(true)
    })
  })

  describe("Scenario 5: Refund with user ATA creation", () => {
    it("measures with common ALT (same tx size — ATA create is CPI)", () => {
      const { header, ixs } = typicalExecuteIxs()
      const r = measureRefund("refund_ata_create_with_alt", header, ixs, alt, 45_000)
      expect(r.transactionBytes).toBeLessThanOrEqual(SOLANA_TX_LIMIT)
    })
  })

  afterAll(() => {
    const artifact = {
      generated: new Date().toISOString(),
      solana_tx_limit: SOLANA_TX_LIMIT,
      scenarios: results,
      summary: {
        total: results.length,
        fits: results.filter((r) => r.classification === "fits").length,
        pressure: results.filter((r) => r.classification === "phase-2 pressure").length,
        blockers: results.filter((r) => r.classification === "blocker").length,
      },
      phase2_notes: [] as string[],
    }

    for (const r of results) {
      if (r.classification === "blocker") {
        artifact.phase2_notes.push(
          `${r.name}: ${r.transactionBytes}B exceeds ${SOLANA_TX_LIMIT}B limit. ` +
          `Consider: reduce MAX_CALLS, reduce MAX_DATA_LEN, use route-specific ALT, ` +
          `or split into bundled transactions.`
        )
      } else if (r.classification === "phase-2 pressure") {
        artifact.phase2_notes.push(
          `${r.name}: ${r.transactionBytes}B is within limit but above 85% ` +
          `(${Math.round(r.transactionBytes / SOLANA_TX_LIMIT * 100)}%). ` +
          `Monitor growth from Jupiter/bridge integrations.`
        )
      }
    }

    if (artifact.phase2_notes.length === 0) {
      artifact.phase2_notes.push(
        "All measured scenarios fit comfortably within Solana tx limits. " +
        "Phase-2 mitigation is not required for POC flows."
      )
    }

    const outPath = resolve(__dirname, "fixtures/tx_size.json")
    writeFileSync(outPath, JSON.stringify(artifact, null, 2) + "\n")
  })
})
