import {
  PublicKey,
  SystemProgram,
  SYSVAR_CLOCK_PUBKEY,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js"
import type { AddressLookupTableAccount } from "@solana/web3.js"
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token"
import { sha256 } from "@noble/hashes/sha256"

import { encodeIntentHeader, decodeIntentHeader } from "./header.js"
import type { IntentHeader } from "./header.js"
import { encodeCalls, NAMED_PREFIX } from "./wire.js"
import type { CallSpecLike } from "./wire.js"
import { deriveIntentPda, deriveSourceAta, deriveReceiverToken } from "./pda.js"

// ---------------------------------------------------------------------------
// Symbolic fixed-slot references
// ---------------------------------------------------------------------------

const FIXED_SLOT_BYTES: Record<string, Uint8Array> = {
  IntentPda: new Uint8Array(32).fill(0xf0),
  SourceAta: new Uint8Array(32).fill(0xf1),
  Receiver: new Uint8Array(32).fill(0xf2),
  ReceiverToken: new Uint8Array(32).fill(0xf3),
}

export const FixedSlot = {
  IntentPda: new PublicKey(FIXED_SLOT_BYTES.IntentPda!),
  SourceAta: new PublicKey(FIXED_SLOT_BYTES.SourceAta!),
  Receiver: new PublicKey(FIXED_SLOT_BYTES.Receiver!),
  ReceiverToken: new PublicKey(FIXED_SLOT_BYTES.ReceiverToken!),
} as const

const FIXED_VINDEX_MAP = new Map<string, number>([
  [FixedSlot.IntentPda.toBase58(), 0],
  [FixedSlot.SourceAta.toBase58(), 1],
  [FixedSlot.Receiver.toBase58(), 2],
  [FixedSlot.ReceiverToken.toBase58(), 3],
])

function isFixedSlot(pk: PublicKey): boolean {
  return FIXED_VINDEX_MAP.has(pk.toBase58())
}

// ---------------------------------------------------------------------------
// Symbolic instruction type
// ---------------------------------------------------------------------------

export interface SymbolicAccountMeta {
  pubkey: PublicKey
  isSigner: boolean
  isWritable: boolean
}

export interface SymbolicInstruction {
  programId: PublicKey
  keys: SymbolicAccountMeta[]
  data: Uint8Array
}

// ---------------------------------------------------------------------------
// Two-pass dedupe: symbolic → CallSpecLike[] + tail PublicKey[]
// ---------------------------------------------------------------------------

export interface DedupeResult {
  calls: CallSpecLike[]
  tailPubkeys: PublicKey[]
}

export function dedupeAccounts(
  symbolicIxs: SymbolicInstruction[]
): DedupeResult {
  const tailMap = new Map<string, number>()
  const tailPubkeys: PublicKey[] = []
  const calls: CallSpecLike[] = []

  for (const ix of symbolicIxs) {
    if (isFixedSlot(ix.programId)) {
      throw new Error(
        "BuilderError: program_id cannot be a fixed-slot reference"
      )
    }

    const programVix = resolveVindex(ix.programId, tailMap, tailPubkeys)

    const accounts: number[] = []
    const isWritable: boolean[] = []
    const isSigner: boolean[] = []

    for (const meta of ix.keys) {
      const vix = resolveVindex(meta.pubkey, tailMap, tailPubkeys)
      accounts.push(vix)
      isWritable.push(meta.isWritable)
      isSigner.push(meta.isSigner)
    }

    calls.push({
      programIx: programVix,
      accounts,
      isWritable,
      isSigner,
      data: ix.data,
    })
  }

  return { calls, tailPubkeys }
}

function resolveVindex(
  pk: PublicKey,
  tailMap: Map<string, number>,
  tailPubkeys: PublicKey[]
): number {
  const fixedVix = FIXED_VINDEX_MAP.get(pk.toBase58())
  if (fixedVix !== undefined) return fixedVix

  const key = pk.toBase58()
  const existing = tailMap.get(key)
  if (existing !== undefined) return existing

  const vix = NAMED_PREFIX + tailPubkeys.length
  tailMap.set(key, vix)
  tailPubkeys.push(pk)
  return vix
}

// ---------------------------------------------------------------------------
// Execute instruction builder
// ---------------------------------------------------------------------------

export interface BuildExecuteIxInput {
  header: IntentHeader
  symbolicIxs: SymbolicInstruction[]
  programId: PublicKey
}

export interface BuildExecuteIxResult {
  executeIx: TransactionInstruction
  intentPda: PublicKey
  sourceAta: PublicKey
  receiverToken: PublicKey
  headerBytes: Uint8Array
  callsBytes: Uint8Array
  tailPubkeys: PublicKey[]
  bump: number
}

export function buildExecuteIntentIx(
  input: BuildExecuteIxInput
): BuildExecuteIxResult {
  const { header, symbolicIxs, programId } = input

  const headerBytes = encodeIntentHeader(header)
  const { calls, tailPubkeys } = dedupeAccounts(symbolicIxs)
  const callsBytes = encodeCalls(calls)

  const [intentPda, bump] = deriveIntentPda(headerBytes, programId)

  const srcMint = header.srcMint
  if (!srcMint) {
    throw new Error("BuilderError: v1 requires srcMint to be set")
  }
  const sourceAta = deriveSourceAta(intentPda, srcMint)

  let receiverToken: PublicKey
  if (header.outMint) {
    receiverToken = deriveReceiverToken(header.receiver, header.outMint)
  } else {
    receiverToken = SystemProgram.programId
  }

  const ixData = buildIxData("execute_intent", headerBytes, callsBytes, bump)

  const keys = [
    { pubkey: intentPda, isSigner: false, isWritable: true },
    { pubkey: sourceAta, isSigner: false, isWritable: true },
    { pubkey: header.receiver, isSigner: false, isWritable: false },
    { pubkey: receiverToken, isSigner: false, isWritable: true },
    { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
    { pubkey: header.executor, isSigner: true, isWritable: false },
  ]

  for (const pk of tailPubkeys) {
    keys.push({ pubkey: pk, isSigner: false, isWritable: false })
  }

  const executeIx = new TransactionInstruction({
    programId,
    keys,
    data: ixData,
  })

  return {
    executeIx,
    intentPda,
    sourceAta,
    receiverToken,
    headerBytes,
    callsBytes,
    tailPubkeys,
    bump,
  }
}

// ---------------------------------------------------------------------------
// Execute transaction builder (delegates to buildExecuteIntentIx)
// ---------------------------------------------------------------------------

export interface BuildExecuteInput {
  header: IntentHeader
  symbolicIxs: SymbolicInstruction[]
  payer: PublicKey
  programId: PublicKey
  lookupTables?: AddressLookupTableAccount[]
}

export interface BuildExecuteResult {
  tx: VersionedTransaction
  intentPda: PublicKey
  sourceAta: PublicKey
  headerBytes: Uint8Array
  callsBytes: Uint8Array
  tailPubkeys: PublicKey[]
  bump: number
}

export function buildExecuteIntentTx(
  input: BuildExecuteInput
): BuildExecuteResult {
  const { payer, lookupTables, ...ixInput } = input

  const { executeIx, ...metadata } = buildExecuteIntentIx(ixInput)

  const message = new TransactionMessage({
    payerKey: payer,
    recentBlockhash: PublicKey.default.toBase58(),
    instructions: [executeIx],
  }).compileToV0Message(lookupTables)

  const tx = new VersionedTransaction(message)

  return { tx, ...metadata }
}

// ---------------------------------------------------------------------------
// Generic TransactionInstruction → SymbolicInstruction converter
// ---------------------------------------------------------------------------

export interface ConvertToSymbolicIxOptions {
  accountMap?: Map<string, PublicKey> | Record<string, PublicKey>
  signerPolicy?: "preserve" | "intent-pda-only"
}

export function convertToSymbolicIx(
  ix: TransactionInstruction,
  options?: ConvertToSymbolicIxOptions
): SymbolicInstruction {
  const map = normalizeAccountMap(options?.accountMap)
  const intentOnly = options?.signerPolicy === "intent-pda-only"

  const keys: SymbolicAccountMeta[] = ix.keys.map((meta) => {
    const resolved = map.get(meta.pubkey.toBase58()) ?? meta.pubkey
    let { isSigner } = meta
    if (intentOnly && isSigner && !resolved.equals(FixedSlot.IntentPda)) {
      isSigner = false
    }
    return { pubkey: resolved, isSigner, isWritable: meta.isWritable }
  })

  return {
    programId: ix.programId,
    keys,
    data: ix.data,
  }
}

function normalizeAccountMap(
  input?: Map<string, PublicKey> | Record<string, PublicKey>
): Map<string, PublicKey> {
  if (!input) return new Map()
  if (input instanceof Map) return input
  return new Map(Object.entries(input))
}

// ---------------------------------------------------------------------------
// Refund builder
// ---------------------------------------------------------------------------

export interface BuildRefundInput {
  headerBytes: Uint8Array
  callsBytes?: Uint8Array
  tailPubkeys?: PublicKey[]
  payer: PublicKey
  programId: PublicKey
  lookupTables?: AddressLookupTableAccount[]
}

export interface BuildRefundResult {
  tx: VersionedTransaction
  intentPda: PublicKey
  sourceAta: PublicKey
  bump: number
}

export function buildRefundIntentTx(
  input: BuildRefundInput
): BuildRefundResult {
  const {
    headerBytes,
    payer,
    programId,
    lookupTables,
  } = input

  const [intentPda, bump] = deriveIntentPda(headerBytes, programId)

  const header = decodeIntentHeader(headerBytes)

  const srcMint = header.srcMint
  if (!srcMint) {
    throw new Error("BuilderError: v1 requires srcMint for refund")
  }

  const sourceAta = deriveSourceAta(intentPda, srcMint)
  const userSourceAta = deriveSourceAta(header.user, srcMint)

  const ixData = buildRefundIxData("refund_intent", headerBytes, bump)

  const keys = [
    { pubkey: intentPda, isSigner: false, isWritable: true },
    { pubkey: sourceAta, isSigner: false, isWritable: true },
    { pubkey: header.user, isSigner: false, isWritable: true },
    { pubkey: userSourceAta, isSigner: false, isWritable: true },
    { pubkey: srcMint, isSigner: false, isWritable: false },
    { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    { pubkey: SYSVAR_CLOCK_PUBKEY, isSigner: false, isWritable: false },
    { pubkey: payer, isSigner: true, isWritable: true },
  ]

  const refundIx = new TransactionInstruction({
    programId,
    keys,
    data: ixData,
  })

  const message = new TransactionMessage({
    payerKey: payer,
    recentBlockhash: PublicKey.default.toBase58(),
    instructions: [refundIx],
  }).compileToV0Message(lookupTables)

  const tx = new VersionedTransaction(message)

  return { tx, intentPda, sourceAta, bump }
}

// ---------------------------------------------------------------------------
// Anchor instruction discriminator + data encoding
// ---------------------------------------------------------------------------

function buildIxData(
  ixName: string,
  headerBytes: Uint8Array,
  callsBytes: Uint8Array,
  bump: number
): Buffer {
  const disc = anchorDiscriminator(ixName)

  const headerLenBuf = Buffer.alloc(4)
  headerLenBuf.writeUInt32LE(headerBytes.length)

  const callsLenBuf = Buffer.alloc(4)
  callsLenBuf.writeUInt32LE(callsBytes.length)

  return Buffer.concat([
    disc,
    headerLenBuf,
    headerBytes,
    callsLenBuf,
    callsBytes,
    Buffer.from([bump]),
  ])
}

function buildRefundIxData(
  ixName: string,
  headerBytes: Uint8Array,
  bump: number
): Buffer {
  const disc = anchorDiscriminator(ixName)

  const headerLenBuf = Buffer.alloc(4)
  headerLenBuf.writeUInt32LE(headerBytes.length)

  return Buffer.concat([
    disc,
    headerLenBuf,
    headerBytes,
    Buffer.from([bump]),
  ])
}

function anchorDiscriminator(ixName: string): Buffer {
  const preimage = `global:${ixName}`
  const hash = sha256(Buffer.from(preimage))
  return Buffer.from(hash.slice(0, 8))
}
