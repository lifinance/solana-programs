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
    outMint: PublicKey.unique(),
    receiver: PublicKey.unique(),
    minAmountOut: 950_000n,
    feeRecipients: [],
    deadline: 1_700_000_000n,
    salt: new Uint8Array(32).fill(0x07),
    executor: null,
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
    const sigCount = header.executor ? 2 : 1
    const staticKeyCount = result.tx.message.staticAccountKeys.length
    const ixDataLen = result.headerBytes.length + result.callsBytes.length + 8 + 1 + 8
    const accountKeyIndexCount = 5 + result.tailPubkeys.length + (header.executor ? 1 : 0)
    msgBytes = 1 + 32 + 32 + staticKeyCount * 32 + 3 + 1 + accountKeyIndexCount + ixDataLen + 4
    txBytes = 1 + sigCount * 64 + msgBytes
  }

  const entry: ScenarioResult = {
    name,
    kind: "execute",
    calls: symbolicIxs.length,
    namedAccounts: 5,
    tailAccounts: result.tailPubkeys.length,
    signatures: header.executor ? 2 : 1,
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
  symbolicIxs: SymbolicInstruction[],
  alt?: AddressLookupTableAccount,
  estimatedCU: number | null = null
): ScenarioResult {
  const headerBytes = encodeIntentHeader(header)
  const { calls, tailPubkeys } = dedupeAccounts(symbolicIxs)
  const callsBytes = encodeCalls(calls)

  const refundResult = buildRefundIntentTx({
    headerBytes,
    callsBytes,
    tailPubkeys,
    payer: PAYER,
    programId: PROGRAM_ID,
    lookupTables: alt ? [alt] : undefined,
  })

  const txBytes = refundResult.tx.serialize().length
  const msgBytes = refundResult.tx.message.serialize().length

  const entry: ScenarioResult = {
    name,
    kind: "refund",
    calls: symbolicIxs.length,
    namedAccounts: 10,
    tailAccounts: tailPubkeys.length,
    signatures: 1,
    headerBytes: headerBytes.length,
    callsBytes: callsBytes.length,
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
      { pubkey: FixedSlot.ReceiverToken, isSigner: false, isWritable: true },
      { pubkey: FixedSlot.IntentPda, isSigner: true, isWritable: false },
    ],
    9
  )

  const ix2 = makeSymIx(
    tokenProgram,
    [
      { pubkey: FixedSlot.SourceAta, isSigner: false, isWritable: true },
      { pubkey: PublicKey.unique(), isSigner: false, isWritable: false },
      { pubkey: FixedSlot.ReceiverToken, isSigner: false, isWritable: true },
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
  const feeRecipient1 = PublicKey.unique()
  const feeRecipient2 = PublicKey.unique()
  const sorted = [feeRecipient1, feeRecipient2].sort((a, b) => {
    const aBytes = a.toBytes()
    const bBytes = b.toBytes()
    for (let i = 0; i < 32; i++) {
      if (aBytes[i]! < bBytes[i]!) return -1
      if (aBytes[i]! > bBytes[i]!) return 1
    }
    return 0
  })

  const header = makeHeader({
    feeRecipients: [
      { pubkey: sorted[0]!, amount: 5000n },
      { pubkey: sorted[1]!, amount: 3000n },
    ],
    executor: PublicKey.unique(),
  })

  const tokenProgram = TOKEN_PROGRAM_ID
  const feeAta1 = PublicKey.unique()
  const feeAta2 = PublicKey.unique()
  const srcMintAcc = PublicKey.unique()

  const transferToFee1 = makeSymIx(
    tokenProgram,
    [
      { pubkey: FixedSlot.SourceAta, isSigner: false, isWritable: true },
      { pubkey: srcMintAcc, isSigner: false, isWritable: false },
      { pubkey: feeAta1, isSigner: false, isWritable: true },
      { pubkey: FixedSlot.IntentPda, isSigner: true, isWritable: false },
    ],
    9
  )

  const transferToFee2 = makeSymIx(
    tokenProgram,
    [
      { pubkey: FixedSlot.SourceAta, isSigner: false, isWritable: true },
      { pubkey: srcMintAcc, isSigner: false, isWritable: false },
      { pubkey: feeAta2, isSigner: false, isWritable: true },
      { pubkey: FixedSlot.IntentPda, isSigner: true, isWritable: false },
    ],
    9
  )

  const transferToReceiver = makeSymIx(
    tokenProgram,
    [
      { pubkey: FixedSlot.SourceAta, isSigner: false, isWritable: true },
      { pubkey: srcMintAcc, isSigner: false, isWritable: false },
      { pubkey: FixedSlot.ReceiverToken, isSigner: false, isWritable: true },
      { pubkey: FixedSlot.IntentPda, isSigner: true, isWritable: false },
    ],
    9
  )

  return { header, ixs: [transferToFee1, transferToFee2, transferToReceiver] }
}

function worstCaseExecuteIxs(): {
  header: IntentHeader
  ixs: SymbolicInstruction[]
} {
  const fees = Array.from({ length: 4 }, () => PublicKey.unique())
  fees.sort((a, b) => {
    const aBytes = a.toBytes()
    const bBytes = b.toBytes()
    for (let i = 0; i < 32; i++) {
      if (aBytes[i]! < bBytes[i]!) return -1
      if (aBytes[i]! > bBytes[i]!) return 1
    }
    return 0
  })

  const header = makeHeader({
    feeRecipients: fees.map((pk, i) => ({
      pubkey: pk,
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
      // may or may not fit — just measure
    })

    it("measures with common ALT", () => {
      const { header, ixs } = worstCaseExecuteIxs()
      const r = measureExecute("worst_case_execute_with_alt", header, ixs, alt, 200_000)
      // flag but don't fail — plan says defer to phase 2 if blocker
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
