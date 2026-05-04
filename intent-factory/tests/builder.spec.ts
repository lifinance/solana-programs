import { describe, it, expect } from "vitest"
import {
  PublicKey,
  SystemProgram,
  SYSVAR_CLOCK_PUBKEY,
  TransactionInstruction,
} from "@solana/web3.js"
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token"
import { readFileSync } from "fs"
import { resolve } from "path"

import {
  FixedSlot,
  dedupeAccounts,
  buildInitIntentIx,
  buildExecuteIntentIx,
  buildExecuteIntentTx,
  buildRefundIntentTx,
  convertToSymbolicIx,
  deriveIntentPda,
  deriveSourceAta,
  encodeIntentHeader,
  encodeCalls,
  computeIntentHash,
  NAMED_PREFIX,
} from "../ts-client/src/index.js"
import type {
  IntentHeader,
  SymbolicInstruction,
  CallSpecLike,
} from "../ts-client/src/index.js"

// ---------------------------------------------------------------------------
// Test helpers
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

function makeSymbolicIx(
  programId: PublicKey,
  keys: Array<{ pubkey: PublicKey; isSigner: boolean; isWritable: boolean }>,
  data: Uint8Array = new Uint8Array()
): SymbolicInstruction {
  return { programId, keys, data }
}

const PROGRAM_ID = PublicKey.unique()

// ---------------------------------------------------------------------------
// Symbolic fixed-slot and dedupe tests
// ---------------------------------------------------------------------------

describe("Symbolic fixed-slot flow", () => {
  it("FixedSlot references map to correct virtual indexes", () => {
    const programId = PublicKey.unique()
    const ix = makeSymbolicIx(programId, [
      { pubkey: FixedSlot.IntentPda, isSigner: true, isWritable: true },
      { pubkey: FixedSlot.SourceAta, isSigner: false, isWritable: true },
    ])

    const { calls, tailPubkeys } = dedupeAccounts([ix])

    expect(calls).toHaveLength(1)
    const call = calls[0]!
    expect(call.accounts).toEqual([0, 1])
    expect(tailPubkeys).toHaveLength(1) // only programId
    expect(tailPubkeys[0]!.equals(programId)).toBe(true)
    expect(call.programIx).toBe(NAMED_PREFIX) // first tail entry
  })

  it("fixed-slot pubkeys do NOT appear in the dynamic tail", () => {
    const programId = PublicKey.unique()
    const tailAccount = PublicKey.unique()

    const ix = makeSymbolicIx(programId, [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
      { pubkey: tailAccount, isSigner: false, isWritable: false },
      { pubkey: FixedSlot.SourceAta, isSigner: false, isWritable: true },
    ])

    const { tailPubkeys } = dedupeAccounts([ix])

    for (const pk of tailPubkeys) {
      expect(pk.equals(FixedSlot.IntentPda)).toBe(false)
      expect(pk.equals(FixedSlot.SourceAta)).toBe(false)
    }
  })

  it("program_id cannot be a fixed-slot reference", () => {
    expect(() =>
      dedupeAccounts([
        makeSymbolicIx(FixedSlot.IntentPda, []),
      ])
    ).toThrow("BuilderError")
  })

  it("duplicate tail accounts are deduped to the same virtual index", () => {
    const programId = PublicKey.unique()
    const shared = PublicKey.unique()

    const ix1 = makeSymbolicIx(programId, [
      { pubkey: shared, isSigner: false, isWritable: true },
    ])
    const ix2 = makeSymbolicIx(programId, [
      { pubkey: shared, isSigner: false, isWritable: false },
    ])

    const { calls, tailPubkeys } = dedupeAccounts([ix1, ix2])

    expect(tailPubkeys.filter((pk) => pk.equals(shared))).toHaveLength(1)

    const sharedVix = calls[0]!.accounts[0]!
    expect(calls[1]!.accounts[0]).toBe(sharedVix)
  })

  it("program_ix is always >= NAMED_PREFIX (in the tail)", () => {
    const programs = [PublicKey.unique(), PublicKey.unique()]
    const ixs = programs.map((pid) =>
      makeSymbolicIx(pid, [
        { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: false },
      ])
    )

    const { calls } = dedupeAccounts(ixs)

    for (const call of calls) {
      expect(call.programIx).toBeGreaterThanOrEqual(NAMED_PREFIX)
    }
  })

  it("per-call signer and writable flags are preserved", () => {
    const programId = PublicKey.unique()
    const account = PublicKey.unique()

    const ix = makeSymbolicIx(programId, [
      { pubkey: FixedSlot.IntentPda, isSigner: true, isWritable: true },
      { pubkey: account, isSigner: false, isWritable: false },
    ])

    const { calls } = dedupeAccounts([ix])
    const call = calls[0]!

    expect(call.isSigner).toEqual([true, false])
    expect(call.isWritable).toEqual([true, false])
  })
})

// ---------------------------------------------------------------------------
// Execute builder tests
// ---------------------------------------------------------------------------

describe("buildExecuteIntentTx", () => {
  it("produces named-prefix account order [intent_pda, source_ata]", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()
    const innerProgram = PublicKey.unique()

    const ix = makeSymbolicIx(innerProgram, [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
    ])

    const result = buildExecuteIntentTx({
      header,
      symbolicIxs: [ix],
      payer: PublicKey.unique(),
      programId,
    })

    const executeIx = result.tx.message.compiledInstructions[0]!
    const staticKeys = result.tx.message.staticAccountKeys

    const intentPdaIx = executeIx.accountKeyIndexes[0]!
    const sourceAtaIx = executeIx.accountKeyIndexes[1]!

    expect(staticKeys[intentPdaIx]!.equals(result.intentPda)).toBe(true)
    expect(staticKeys[sourceAtaIx]!.equals(result.sourceAta)).toBe(true)
  })

  it("includes executor as a transaction signer", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()
    const innerProgram = PublicKey.unique()

    const ix = makeSymbolicIx(innerProgram, [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
    ])

    const result = buildExecuteIntentTx({
      header,
      symbolicIxs: [ix],
      payer: PublicKey.unique(),
      programId,
    })

    const staticKeys = result.tx.message.staticAccountKeys
    const executorInKeys = staticKeys.some((k) => k.equals(header.executor))
    expect(executorInKeys).toBe(true)
  })

  it("executor is NOT marked as inner CPI signer", () => {
    const executor = PublicKey.unique()
    const header = makeHeader({ executor })
    const programId = PublicKey.unique()
    const innerProgram = PublicKey.unique()

    const ix = makeSymbolicIx(innerProgram, [
      { pubkey: FixedSlot.IntentPda, isSigner: true, isWritable: true },
    ])

    const { calls } = dedupeAccounts([ix])

    for (const call of calls) {
      for (let i = 0; i < call.accounts.length; i++) {
        if (call.isSigner[i]) {
          expect(call.accounts[i]).toBe(0)
        }
      }
    }
  })

  it("rejects srcMint = null", () => {
    const header = makeHeader({ srcMint: null })
    const ix = makeSymbolicIx(PublicKey.unique(), [])

    expect(() =>
      buildExecuteIntentTx({
        header,
        symbolicIxs: [ix],
        payer: PublicKey.unique(),
        programId: PublicKey.unique(),
      })
    ).toThrow("BuilderError")
  })

  it("headerBytes match what hash expects", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()
    const innerProgram = PublicKey.unique()

    const ix = makeSymbolicIx(innerProgram, [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
    ])

    const result = buildExecuteIntentTx({
      header,
      symbolicIxs: [ix],
      payer: PublicKey.unique(),
      programId,
    })

    const expectedHash = computeIntentHash(result.headerBytes)

    const [expectedPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("intent"), expectedHash],
      programId
    )

    expect(result.intentPda.equals(expectedPda)).toBe(true)
  })

  it("same header produces same PDA across different symbolic routes", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()

    const ixA = makeSymbolicIx(PublicKey.unique(), [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
      { pubkey: PublicKey.unique(), isSigner: false, isWritable: false },
    ])

    const ixB = makeSymbolicIx(PublicKey.unique(), [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
      { pubkey: PublicKey.unique(), isSigner: false, isWritable: true },
      { pubkey: PublicKey.unique(), isSigner: false, isWritable: false },
    ])

    const resultA = buildExecuteIntentTx({
      header,
      symbolicIxs: [ixA],
      payer: PublicKey.unique(),
      programId,
    })

    const resultB = buildExecuteIntentTx({
      header,
      symbolicIxs: [ixB],
      payer: PublicKey.unique(),
      programId,
    })

    expect(resultA.intentPda.equals(resultB.intentPda)).toBe(true)
    expect(resultA.bump).toBe(resultB.bump)
  })

  it("different salts produce different PDAs", () => {
    const programId = PublicKey.unique()
    const innerProgram = PublicKey.unique()

    const headerA = makeHeader({ salt: new Uint8Array(32).fill(0x01) })
    const headerB = makeHeader({
      ...headerA,
      salt: new Uint8Array(32).fill(0x02),
    })

    const ix = makeSymbolicIx(innerProgram, [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
    ])

    const resultA = buildExecuteIntentTx({
      header: headerA,
      symbolicIxs: [ix],
      payer: PublicKey.unique(),
      programId,
    })

    const resultB = buildExecuteIntentTx({
      header: headerB,
      symbolicIxs: [ix],
      payer: PublicKey.unique(),
      programId,
    })

    expect(resultA.intentPda.equals(resultB.intentPda)).toBe(false)
  })

  it("tail accounts appear after named-prefix in instruction keys", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()
    const innerProgram = PublicKey.unique()
    const tailAcc1 = PublicKey.unique()
    const tailAcc2 = PublicKey.unique()

    const ix = makeSymbolicIx(innerProgram, [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
      { pubkey: tailAcc1, isSigner: false, isWritable: false },
      { pubkey: tailAcc2, isSigner: false, isWritable: false },
    ])

    const result = buildExecuteIntentTx({
      header,
      symbolicIxs: [ix],
      payer: PublicKey.unique(),
      programId,
    })

    expect(result.tailPubkeys.some((pk) => pk.equals(innerProgram))).toBe(true)
    expect(result.tailPubkeys.some((pk) => pk.equals(tailAcc1))).toBe(true)
    expect(result.tailPubkeys.some((pk) => pk.equals(tailAcc2))).toBe(true)
  })
})

// ---------------------------------------------------------------------------
// Execute instruction builder tests
// ---------------------------------------------------------------------------

describe("buildExecuteIntentIx", () => {
  it("returns execute instruction with correct account ordering", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()
    const innerProgram = PublicKey.unique()

    const ix = makeSymbolicIx(innerProgram, [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
    ])

    const result = buildExecuteIntentIx({
      header,
      symbolicIxs: [ix],
      programId,
    })

    expect(result.executeIx.programId.equals(programId)).toBe(true)
    expect(result.executeIx.keys[0]!.pubkey.equals(result.intentPda)).toBe(true)
    expect(result.executeIx.keys[1]!.pubkey.equals(result.sourceAta)).toBe(true)
    expect(result.executeIx.keys[2]!.pubkey.equals(SYSVAR_CLOCK_PUBKEY)).toBe(true)
    expect(result.executeIx.keys[3]!.pubkey.equals(header.executor)).toBe(true)
    expect(result.executeIx.keys[3]!.isSigner).toBe(true)
  })

  it("produces same PDA and metadata as buildExecuteIntentTx", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()
    const innerProgram = PublicKey.unique()

    const ix = makeSymbolicIx(innerProgram, [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
    ])

    const ixResult = buildExecuteIntentIx({
      header,
      symbolicIxs: [ix],
      programId,
    })

    const txResult = buildExecuteIntentTx({
      header,
      symbolicIxs: [ix],
      payer: PublicKey.unique(),
      programId,
    })

    expect(ixResult.intentPda.equals(txResult.intentPda)).toBe(true)
    expect(ixResult.sourceAta.equals(txResult.sourceAta)).toBe(true)
    expect(ixResult.bump).toBe(txResult.bump)
    expect(Buffer.from(ixResult.headerBytes).equals(Buffer.from(txResult.headerBytes))).toBe(true)
    expect(Buffer.from(ixResult.callsBytes).equals(Buffer.from(txResult.callsBytes))).toBe(true)
    expect(ixResult.tailPubkeys.length).toBe(txResult.tailPubkeys.length)
  })

  it("executor is the named fourth account (index 3) with isSigner=true", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()

    const ix = makeSymbolicIx(PublicKey.unique(), [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
    ])

    const result = buildExecuteIntentIx({
      header,
      symbolicIxs: [ix],
      programId,
    })

    expect(result.executeIx.keys[3]!.pubkey.equals(header.executor)).toBe(true)
    expect(result.executeIx.keys[3]!.isSigner).toBe(true)
    expect(result.executeIx.keys[3]!.isWritable).toBe(false)
  })

  it("rejects srcMint = null", () => {
    const header = makeHeader({ srcMint: null })
    const ix = makeSymbolicIx(PublicKey.unique(), [])

    expect(() =>
      buildExecuteIntentIx({
        header,
        symbolicIxs: [ix],
        programId: PublicKey.unique(),
      })
    ).toThrow("BuilderError")
  })

  it("outcome accounts are appended to the tail and marked writable", () => {
    const outcomeAccount = PublicKey.unique()
    const header = makeHeader({
      outcomes: [
        { mint: PublicKey.unique(), account: outcomeAccount, amount: 950_000n },
      ],
    })
    const programId = PublicKey.unique()

    const ix = makeSymbolicIx(PublicKey.unique(), [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
    ])

    const result = buildExecuteIntentIx({ header, symbolicIxs: [ix], programId })

    const outcomeKey = result.executeIx.keys.find((k) => k.pubkey.equals(outcomeAccount))
    expect(outcomeKey).toBeDefined()
    expect(outcomeKey!.isWritable).toBe(true)
  })

  it("outcome account already in route tail is not duplicated but forced writable", () => {
    const outcomeAccount = PublicKey.unique()
    const header = makeHeader({
      outcomes: [
        { mint: PublicKey.unique(), account: outcomeAccount, amount: 950_000n },
      ],
    })
    const programId = PublicKey.unique()

    const ix = makeSymbolicIx(PublicKey.unique(), [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
      { pubkey: outcomeAccount, isSigner: false, isWritable: false },
    ])

    const result = buildExecuteIntentIx({ header, symbolicIxs: [ix], programId })

    const matchingKeys = result.executeIx.keys.filter((k) => k.pubkey.equals(outcomeAccount))
    expect(matchingKeys).toHaveLength(1)
    expect(matchingKeys[0]!.isWritable).toBe(true)
  })

  it("no outcomes produces no extra remaining accounts for outcomes", () => {
    const header = makeHeader({ outcomes: [] })
    const programId = PublicKey.unique()

    const ix = makeSymbolicIx(PublicKey.unique(), [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
    ])

    const result = buildExecuteIntentIx({ header, symbolicIxs: [ix], programId })

    expect(result.executeIx.keys).toHaveLength(4 + result.tailPubkeys.length)
  })
})

// ---------------------------------------------------------------------------
// Init instruction builder tests
// ---------------------------------------------------------------------------

describe("buildInitIntentIx", () => {
  it("returns init instruction with correct account ordering", () => {
    const header = makeHeader()
    const payer = PublicKey.unique()
    const programId = PublicKey.unique()

    const result = buildInitIntentIx({ header, payer, programId })

    expect(result.initIx.programId.equals(programId)).toBe(true)
    expect(result.initIx.keys[0]!.pubkey.equals(result.intentPda)).toBe(true)
    expect(result.initIx.keys[1]!.pubkey.equals(header.executor)).toBe(true)
    expect(result.initIx.keys[2]!.pubkey.equals(payer)).toBe(true)
    expect(result.initIx.keys[3]!.pubkey.equals(SystemProgram.programId)).toBe(true)
  })

  it("executor is signer, payer is signer+writable", () => {
    const header = makeHeader()
    const result = buildInitIntentIx({ header, payer: PublicKey.unique(), programId: PublicKey.unique() })

    expect(result.initIx.keys[1]!.isSigner).toBe(true)
    expect(result.initIx.keys[1]!.isWritable).toBe(false)
    expect(result.initIx.keys[2]!.isSigner).toBe(true)
    expect(result.initIx.keys[2]!.isWritable).toBe(true)
  })

  it("PDA matches deriveIntentPda from headerBytes", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()

    const result = buildInitIntentIx({ header, payer: PublicKey.unique(), programId })
    const [manualPda, manualBump] = deriveIntentPda(result.headerBytes, programId)

    expect(result.intentPda.equals(manualPda)).toBe(true)
    expect(result.bump).toBe(manualBump)
  })

  it("instruction data layout: disc | u32 header_len | header_bytes | u8 bump", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()

    const result = buildInitIntentIx({ header, payer: PublicKey.unique(), programId })
    const data = result.initIx.data
    let cursor = 8

    const headerLen = data.readUInt32LE(cursor)
    cursor += 4
    expect(headerLen).toBe(result.headerBytes.length)
    expect(
      Buffer.from(data.subarray(cursor, cursor + headerLen)).equals(Buffer.from(result.headerBytes))
    ).toBe(true)
    cursor += headerLen

    expect(data[cursor]).toBe(result.bump)
    cursor += 1

    expect(cursor).toBe(data.length)
  })

  it("has exactly 4 account keys", () => {
    const header = makeHeader()
    const result = buildInitIntentIx({ header, payer: PublicKey.unique(), programId: PublicKey.unique() })
    expect(result.initIx.keys).toHaveLength(4)
  })
})

// ---------------------------------------------------------------------------
// Dedupe writable tail metadata tests
// ---------------------------------------------------------------------------

describe("dedupeAccounts tail writable metadata", () => {
  it("readonly tail accounts stay readonly", () => {
    const programId = PublicKey.unique()
    const account = PublicKey.unique()

    const ix = makeSymbolicIx(programId, [
      { pubkey: account, isSigner: false, isWritable: false },
    ])

    const { tailIsWritable } = dedupeAccounts([ix])

    const accountIdx = tailIsWritable.length - 1
    expect(tailIsWritable[accountIdx]).toBe(false)
  })

  it("writable tail accounts are marked writable", () => {
    const programId = PublicKey.unique()
    const account = PublicKey.unique()

    const ix = makeSymbolicIx(programId, [
      { pubkey: account, isSigner: false, isWritable: true },
    ])

    const { tailIsWritable, tailPubkeys } = dedupeAccounts([ix])

    const accountIdx = tailPubkeys.findIndex((pk) => pk.equals(account))
    expect(tailIsWritable[accountIdx]).toBe(true)
  })

  it("shared accounts use the union of writable flags", () => {
    const programId = PublicKey.unique()
    const shared = PublicKey.unique()

    const ix1 = makeSymbolicIx(programId, [
      { pubkey: shared, isSigner: false, isWritable: false },
    ])
    const ix2 = makeSymbolicIx(programId, [
      { pubkey: shared, isSigner: false, isWritable: true },
    ])

    const { tailIsWritable, tailPubkeys } = dedupeAccounts([ix1, ix2])

    const sharedIdx = tailPubkeys.findIndex((pk) => pk.equals(shared))
    expect(tailIsWritable[sharedIdx]).toBe(true)
  })

  it("writable tail accounts appear writable in execute instruction keys", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()
    const writableAcc = PublicKey.unique()

    const ix = makeSymbolicIx(PublicKey.unique(), [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
      { pubkey: writableAcc, isSigner: false, isWritable: true },
    ])

    const result = buildExecuteIntentIx({ header, symbolicIxs: [ix], programId })

    const tailKey = result.executeIx.keys.find((k) => k.pubkey.equals(writableAcc))
    expect(tailKey).toBeDefined()
    expect(tailKey!.isWritable).toBe(true)
  })

  it("readonly tail accounts appear readonly in execute instruction keys", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()
    const readonlyAcc = PublicKey.unique()

    const ix = makeSymbolicIx(PublicKey.unique(), [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
      { pubkey: readonlyAcc, isSigner: false, isWritable: false },
    ])

    const result = buildExecuteIntentIx({ header, symbolicIxs: [ix], programId })

    const tailKey = result.executeIx.keys.find((k) => k.pubkey.equals(readonlyAcc))
    expect(tailKey).toBeDefined()
    expect(tailKey!.isWritable).toBe(false)
  })
})

// ---------------------------------------------------------------------------
// convertToSymbolicIx tests
// ---------------------------------------------------------------------------

describe("convertToSymbolicIx", () => {
  it("preserves programId, keys, flags, and data by default", () => {
    const programId = PublicKey.unique()
    const account = PublicKey.unique()
    const data = Buffer.from([1, 2, 3])

    const txIx = new TransactionInstruction({
      programId,
      keys: [{ pubkey: account, isSigner: true, isWritable: true }],
      data,
    })

    const sym = convertToSymbolicIx(txIx)

    expect(sym.programId.equals(programId)).toBe(true)
    expect(sym.keys).toHaveLength(1)
    expect(sym.keys[0]!.pubkey.equals(account)).toBe(true)
    expect(sym.keys[0]!.isSigner).toBe(true)
    expect(sym.keys[0]!.isWritable).toBe(true)
    expect(Buffer.from(sym.data).equals(data)).toBe(true)
  })

  it("applies accountMap substitutions using a Map", () => {
    const original = PublicKey.unique()
    const replacement = FixedSlot.IntentPda

    const txIx = new TransactionInstruction({
      programId: PublicKey.unique(),
      keys: [{ pubkey: original, isSigner: false, isWritable: true }],
      data: Buffer.alloc(0),
    })

    const map = new Map([[original.toBase58(), replacement]])
    const sym = convertToSymbolicIx(txIx, { accountMap: map })

    expect(sym.keys[0]!.pubkey.equals(replacement)).toBe(true)
  })

  it("applies accountMap substitutions using a Record", () => {
    const original = PublicKey.unique()
    const replacement = FixedSlot.SourceAta

    const txIx = new TransactionInstruction({
      programId: PublicKey.unique(),
      keys: [{ pubkey: original, isSigner: false, isWritable: false }],
      data: Buffer.alloc(0),
    })

    const sym = convertToSymbolicIx(txIx, {
      accountMap: { [original.toBase58()]: replacement },
    })

    expect(sym.keys[0]!.pubkey.equals(replacement)).toBe(true)
  })

  it("intent-pda-only policy clears non-intent signers", () => {
    const walletSigner = PublicKey.unique()
    const intentPda = FixedSlot.IntentPda

    const txIx = new TransactionInstruction({
      programId: PublicKey.unique(),
      keys: [
        { pubkey: walletSigner, isSigner: true, isWritable: false },
        { pubkey: intentPda, isSigner: true, isWritable: true },
      ],
      data: Buffer.alloc(0),
    })

    const sym = convertToSymbolicIx(txIx, { signerPolicy: "intent-pda-only" })

    expect(sym.keys[0]!.isSigner).toBe(false)
    expect(sym.keys[1]!.isSigner).toBe(true)
  })

  it("intent-pda-only policy works with accountMap substitution", () => {
    const placeholder = PublicKey.unique()

    const txIx = new TransactionInstruction({
      programId: PublicKey.unique(),
      keys: [
        { pubkey: placeholder, isSigner: true, isWritable: true },
        { pubkey: PublicKey.unique(), isSigner: true, isWritable: false },
      ],
      data: Buffer.alloc(0),
    })

    const sym = convertToSymbolicIx(txIx, {
      accountMap: new Map([[placeholder.toBase58(), FixedSlot.IntentPda]]),
      signerPolicy: "intent-pda-only",
    })

    expect(sym.keys[0]!.pubkey.equals(FixedSlot.IntentPda)).toBe(true)
    expect(sym.keys[0]!.isSigner).toBe(true)
    expect(sym.keys[1]!.isSigner).toBe(false)
  })

  it("preserves writable flags regardless of signer policy", () => {
    const txIx = new TransactionInstruction({
      programId: PublicKey.unique(),
      keys: [
        { pubkey: PublicKey.unique(), isSigner: true, isWritable: true },
        { pubkey: PublicKey.unique(), isSigner: false, isWritable: false },
      ],
      data: Buffer.alloc(0),
    })

    const sym = convertToSymbolicIx(txIx, { signerPolicy: "intent-pda-only" })

    expect(sym.keys[0]!.isWritable).toBe(true)
    expect(sym.keys[1]!.isWritable).toBe(false)
  })
})

// ---------------------------------------------------------------------------
// Refund builder tests
// ---------------------------------------------------------------------------

describe("buildRefundIntentTx", () => {
  it("recomputes the same PDA from headerBytes only", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()
    const innerProgram = PublicKey.unique()

    const ix = makeSymbolicIx(innerProgram, [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
    ])

    const execResult = buildExecuteIntentTx({
      header,
      symbolicIxs: [ix],
      payer: PublicKey.unique(),
      programId,
    })

    const refundResult = buildRefundIntentTx({
      headerBytes: execResult.headerBytes,
      payer: PublicKey.unique(),
      programId,
    })

    expect(refundResult.intentPda.equals(execResult.intentPda)).toBe(true)
    expect(refundResult.sourceAta.equals(execResult.sourceAta)).toBe(true)
    expect(refundResult.bump).toBe(execResult.bump)
  })

  it("includes canonical refund accounts in correct order", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()

    const headerBytes = encodeIntentHeader(header)

    const payer = PublicKey.unique()
    const refundResult = buildRefundIntentTx({
      headerBytes,
      payer,
      programId,
    })

    const refundIx = refundResult.tx.message.compiledInstructions[0]!
    const staticKeys = refundResult.tx.message.staticAccountKeys

    const keyAt = (i: number) => staticKeys[refundIx.accountKeyIndexes[i]!]!

    expect(keyAt(0).equals(refundResult.intentPda)).toBe(true)
    expect(keyAt(1).equals(refundResult.sourceAta)).toBe(true)
    expect(keyAt(2).equals(header.user)).toBe(true)

    const userSourceAta = deriveSourceAta(header.user, header.srcMint!)
    expect(keyAt(3).equals(userSourceAta)).toBe(true)
    expect(keyAt(4).equals(header.srcMint!)).toBe(true)
    expect(keyAt(5).equals(TOKEN_PROGRAM_ID)).toBe(true)
    expect(keyAt(6).equals(ASSOCIATED_TOKEN_PROGRAM_ID)).toBe(true)
    expect(keyAt(7).equals(SystemProgram.programId)).toBe(true)
    expect(keyAt(8).equals(SYSVAR_CLOCK_PUBKEY)).toBe(true)
    expect(keyAt(9).equals(payer)).toBe(true)
  })

  it("rejects srcMint = null", () => {
    const header = makeHeader({ srcMint: null })
    const headerBytes = encodeIntentHeader(header)

    expect(() =>
      buildRefundIntentTx({
        headerBytes,
        payer: PublicKey.unique(),
        programId: PublicKey.unique(),
      })
    ).toThrow("BuilderError")
  })

  it("refund PDA is stable across different payers", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()

    const headerBytes = encodeIntentHeader(header)

    const refundA = buildRefundIntentTx({
      headerBytes,
      payer: PublicKey.unique(),
      programId,
    })

    const refundB = buildRefundIntentTx({
      headerBytes,
      payer: PublicKey.unique(),
      programId,
    })

    expect(refundA.intentPda.equals(refundB.intentPda)).toBe(true)
    expect(refundA.bump).toBe(refundB.bump)
  })
})

// ---------------------------------------------------------------------------
// Conformance: builder PDA matches fixture hash
// ---------------------------------------------------------------------------

describe("Builder conformance with fixtures", () => {
  it("execute builder PDA matches manually derived PDA", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()
    const innerProgram = PublicKey.unique()

    const ix = makeSymbolicIx(innerProgram, [
      { pubkey: FixedSlot.IntentPda, isSigner: true, isWritable: true },
      { pubkey: FixedSlot.SourceAta, isSigner: false, isWritable: true },
    ])

    const result = buildExecuteIntentTx({
      header,
      symbolicIxs: [ix],
      payer: PublicKey.unique(),
      programId,
    })

    const [manualPda, manualBump] = deriveIntentPda(
      result.headerBytes,
      programId
    )

    expect(result.intentPda.equals(manualPda)).toBe(true)
    expect(result.bump).toBe(manualBump)
  })
})

// ---------------------------------------------------------------------------
// IDL conformance helpers
// ---------------------------------------------------------------------------

interface IdlAccount {
  name: string
  writable?: boolean
  signer?: boolean
  address?: string
}

interface IdlArg {
  name: string
  type: string
}

interface IdlInstruction {
  name: string
  discriminator: number[]
  accounts: IdlAccount[]
  args: IdlArg[]
}

interface IdlFile {
  address: string
  instructions: IdlInstruction[]
}

const idlPath = resolve(__dirname, "../target/idl/intent_factory.json")
const idl: IdlFile = JSON.parse(readFileSync(idlPath, "utf-8"))

function getIdlIx(name: string): IdlInstruction {
  const ix = idl.instructions.find((i) => i.name === name)
  if (!ix) throw new Error(`IDL instruction "${name}" not found`)
  return ix
}

function assertDiscriminator(ixData: Buffer, expected: number[]): void {
  const actual = Array.from(ixData.subarray(0, 8))
  expect(actual).toEqual(expected)
}

function assertExecutePayloadLayout(
  ixData: Buffer,
  callsBytes: Uint8Array,
): void {
  let cursor = 8

  const callsLen = ixData.readUInt32LE(cursor)
  cursor += 4
  expect(callsLen).toBe(callsBytes.length)
  expect(
    Buffer.from(ixData.subarray(cursor, cursor + callsLen)).equals(Buffer.from(callsBytes))
  ).toBe(true)
  cursor += callsLen

  expect(cursor).toBe(ixData.length)
}

function assertRefundPayloadLayout(
  ixData: Buffer,
): void {
  expect(ixData.length).toBe(8)
}

function assertExecuteIdlArgs(idlIx: IdlInstruction): void {
  expect(idlIx.args).toHaveLength(1)
  expect(idlIx.args[0]).toEqual({ name: "calls_bytes", type: "bytes" })
}

function assertRefundIdlArgs(idlIx: IdlInstruction): void {
  expect(idlIx.args).toHaveLength(0)
}

function assertAccountMeta(
  actual: { pubkey: PublicKey; isSigner: boolean; isWritable: boolean },
  idlAccount: IdlAccount,
  label: string,
): void {
  expect(actual.isWritable).toBe(
    idlAccount.writable ?? false,
  )
  expect(actual.isSigner).toBe(
    idlAccount.signer ?? false,
  )

  if (idlAccount.address) {
    expect(
      actual.pubkey.equals(new PublicKey(idlAccount.address))
    ).toBe(true)
  }
}

// ---------------------------------------------------------------------------
// IDL conformance: buildInitIntentIx
// ---------------------------------------------------------------------------

describe("IDL conformance: buildInitIntentIx", () => {
  const idlIx = getIdlIx("init_intent")

  it("IDL arg shape is [header_bytes: bytes, bump: u8]", () => {
    expect(idlIx.args).toHaveLength(2)
    expect(idlIx.args[0]).toEqual({ name: "header_bytes", type: "bytes" })
    expect(idlIx.args[1]).toEqual({ name: "bump", type: "u8" })
  })

  it("discriminator matches IDL", () => {
    const header = makeHeader()
    const result = buildInitIntentIx({ header, payer: PublicKey.unique(), programId: PublicKey.unique() })
    assertDiscriminator(result.initIx.data, idlIx.discriminator)
  })

  it("fixed account count matches IDL (4 accounts)", () => {
    expect(idlIx.accounts).toHaveLength(4)
  })

  it("fixed account order and flags match IDL", () => {
    const header = makeHeader()
    const result = buildInitIntentIx({ header, payer: PublicKey.unique(), programId: PublicKey.unique() })
    const keys = result.initIx.keys

    for (let i = 0; i < idlIx.accounts.length; i++) {
      const idlAcc = idlIx.accounts[i]!
      const key = keys[i]!
      assertAccountMeta(key, idlAcc, idlAcc.name)
    }
  })

  it("system_program address matches IDL constraint", () => {
    const header = makeHeader()
    const result = buildInitIntentIx({ header, payer: PublicKey.unique(), programId: PublicKey.unique() })

    const sysIdx = idlIx.accounts.findIndex((a) => a.name === "system_program")
    const sysIdlAddr = idlIx.accounts[sysIdx]!.address!

    expect(result.initIx.keys[sysIdx]!.pubkey.equals(new PublicKey(sysIdlAddr))).toBe(true)
  })

  it("executor is marked as signer in IDL", () => {
    const executorAcc = idlIx.accounts.find((a) => a.name === "executor")
    expect(executorAcc).toBeDefined()
    expect(executorAcc!.signer).toBe(true)
  })
})

// ---------------------------------------------------------------------------
// IDL conformance: buildExecuteIntentIx
// ---------------------------------------------------------------------------

describe("IDL conformance: buildExecuteIntentIx", () => {
  const idlIx = getIdlIx("execute_intent")

  it("IDL arg shape is [calls_bytes: bytes]", () => {
    assertExecuteIdlArgs(idlIx)
  })

  it("discriminator matches IDL", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()
    const ix = makeSymbolicIx(PublicKey.unique(), [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
    ])

    const result = buildExecuteIntentIx({ header, symbolicIxs: [ix], programId })
    assertDiscriminator(result.executeIx.data, idlIx.discriminator)
  })

  it("instruction data follows Anchor layout: disc | u32 calls | callsBytes", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()
    const ix = makeSymbolicIx(PublicKey.unique(), [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
      { pubkey: PublicKey.unique(), isSigner: false, isWritable: false },
    ])

    const result = buildExecuteIntentIx({ header, symbolicIxs: [ix], programId })
    assertExecutePayloadLayout(
      result.executeIx.data,
      result.callsBytes,
    )
  })

  it("fixed account count matches IDL", () => {
    expect(idlIx.accounts).toHaveLength(4)
  })

  it("fixed account order and flags match IDL", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()
    const ix = makeSymbolicIx(PublicKey.unique(), [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
    ])

    const result = buildExecuteIntentIx({ header, symbolicIxs: [ix], programId })
    const keys = result.executeIx.keys

    for (let i = 0; i < idlIx.accounts.length; i++) {
      const idlAcc = idlIx.accounts[i]!
      const key = keys[i]!
      assertAccountMeta(key, idlAcc, idlAcc.name)
    }
  })

  it("clock account address matches IDL constraint", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()
    const ix = makeSymbolicIx(PublicKey.unique(), [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
    ])

    const result = buildExecuteIntentIx({ header, symbolicIxs: [ix], programId })
    const clockIdx = idlIx.accounts.findIndex((a) => a.name === "clock")
    const clockIdlAddr = idlIx.accounts[clockIdx]!.address!

    expect(result.executeIx.keys[clockIdx]!.pubkey.equals(new PublicKey(clockIdlAddr))).toBe(true)
  })

  it("tail accounts begin immediately after IDL-declared accounts", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()
    const tailAcc = PublicKey.unique()
    const innerProgram = PublicKey.unique()
    const ix = makeSymbolicIx(innerProgram, [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
      { pubkey: tailAcc, isSigner: false, isWritable: false },
    ])

    const result = buildExecuteIntentIx({ header, symbolicIxs: [ix], programId })
    const idlAccountCount = idlIx.accounts.length

    expect(result.executeIx.keys.length).toBeGreaterThan(idlAccountCount)

    const tailKeys = result.executeIx.keys.slice(idlAccountCount)
    for (const tk of tailKeys) {
      const isInTail = result.tailPubkeys.some((pk) => pk.equals(tk.pubkey))
      expect(isInTail).toBe(true)
    }
  })

  it("executor does not appear in tailPubkeys unless referenced by a route instruction", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()

    const ix = makeSymbolicIx(PublicKey.unique(), [
      { pubkey: FixedSlot.IntentPda, isSigner: false, isWritable: true },
    ])

    const result = buildExecuteIntentIx({ header, symbolicIxs: [ix], programId })

    const executorInTail = result.tailPubkeys.some((pk) => pk.equals(header.executor))
    expect(executorInTail).toBe(false)
  })
})

// ---------------------------------------------------------------------------
// IDL conformance: buildRefundIntentTx
// ---------------------------------------------------------------------------

describe("IDL conformance: buildRefundIntentTx", () => {
  const idlIx = getIdlIx("refund_intent")

  it("IDL arg shape is [] (no args)", () => {
    assertRefundIdlArgs(idlIx)
  })

  it("fixed account count matches IDL (10 accounts)", () => {
    expect(idlIx.accounts).toHaveLength(10)
  })

  it("discriminator matches IDL", () => {
    const header = makeHeader()
    const headerBytes = encodeIntentHeader(header)

    const result = buildRefundIntentTx({
      headerBytes,
      payer: PublicKey.unique(),
      programId: PublicKey.unique(),
    })

    const compiled = result.tx.message.compiledInstructions[0]!
    const disc = Array.from(compiled.data.subarray(0, 8))
    expect(disc).toEqual(idlIx.discriminator)
  })

  it("instruction data follows Anchor layout (disc only)", () => {
    const header = makeHeader()
    const headerBytes = encodeIntentHeader(header)

    const result = buildRefundIntentTx({
      headerBytes,
      payer: PublicKey.unique(),
      programId: PublicKey.unique(),
    })

    const compiled = result.tx.message.compiledInstructions[0]!
    assertRefundPayloadLayout(Buffer.from(compiled.data))
  })

  it("static address constraints match IDL for token_program, ata_program, system_program, clock", () => {
    const header = makeHeader()
    const headerBytes = encodeIntentHeader(header)

    const result = buildRefundIntentTx({
      headerBytes,
      payer: PublicKey.unique(),
      programId: PublicKey.unique(),
    })

    const compiled = result.tx.message.compiledInstructions[0]!
    const staticKeys = result.tx.message.staticAccountKeys

    const addressConstraints = idlIx.accounts
      .map((a, i) => ({ ...a, idx: i }))
      .filter((a) => a.address)

    for (const ac of addressConstraints) {
      const keyIdx = compiled.accountKeyIndexes[ac.idx]!
      const resolvedKey = staticKeys[keyIdx]!
      expect(resolvedKey.equals(new PublicKey(ac.address!))).toBe(true)
    }
  })

  it("refund account count equals IDL-declared accounts only (no tail)", () => {
    const header = makeHeader()
    const programId = PublicKey.unique()
    const headerBytes = encodeIntentHeader(header)

    const result = buildRefundIntentTx({
      headerBytes,
      payer: PublicKey.unique(),
      programId,
    })

    const compiled = result.tx.message.compiledInstructions[0]!
    const idlAccountCount = idlIx.accounts.length

    expect(compiled.accountKeyIndexes.length).toBe(idlAccountCount)
  })
})
