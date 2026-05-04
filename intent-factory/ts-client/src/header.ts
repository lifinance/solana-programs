import { PublicKey } from "@solana/web3.js"

export const MAX_OUTCOMES = 4

export interface IntentOutcome {
  mint: PublicKey | null
  account: PublicKey
  amount: bigint
}

export interface IntentHeader {
  user: PublicKey
  srcMint: PublicKey | null
  amountIn: bigint
  outcomes: IntentOutcome[]
  deadline: bigint
  salt: Uint8Array
  executor: PublicKey
}

export function encodeIntentHeader(header: IntentHeader): Uint8Array {
  const outcomeBytes = header.outcomes.reduce(
    (sum, o) => sum + (o.mint ? 33 : 1) + 32 + 8,
    0
  )
  const capacity = 32 + 33 + 8 + 1 + outcomeBytes + 8 + 32 + 32
  const buf = new Uint8Array(capacity)
  const view = new DataView(buf.buffer)
  let offset = 0

  buf.set(header.user.toBytes(), offset)
  offset += 32

  offset = writeOptionPubkey(buf, offset, header.srcMint)

  view.setBigUint64(offset, header.amountIn, true)
  offset += 8

  buf[offset++] = header.outcomes.length
  for (const outcome of header.outcomes) {
    offset = writeOptionPubkey(buf, offset, outcome.mint)
    buf.set(outcome.account.toBytes(), offset)
    offset += 32
    view.setBigUint64(offset, outcome.amount, true)
    offset += 8
  }

  view.setBigInt64(offset, header.deadline, true)
  offset += 8

  buf.set(header.salt, offset)
  offset += 32

  buf.set(header.executor.toBytes(), offset)
  offset += 32

  return buf.slice(0, offset)
}

export function decodeIntentHeader(bytes: Uint8Array): IntentHeader {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  let offset = 0

  const user = readPubkey(bytes, offset)
  offset += 32

  const [srcMint, srcMintEnd] = readOptionPubkey(bytes, offset)
  offset = srcMintEnd

  const amountIn = view.getBigUint64(offset, true)
  offset += 8

  const outcomeCount = bytes[offset++]!
  if (outcomeCount > MAX_OUTCOMES) {
    throw new Error(`MalformedHeader: outcome_count ${outcomeCount} > ${MAX_OUTCOMES}`)
  }

  const outcomes: IntentOutcome[] = []
  for (let i = 0; i < outcomeCount; i++) {
    const [mint, mintEnd] = readOptionPubkey(bytes, offset)
    offset = mintEnd
    const account = readPubkey(bytes, offset)
    offset += 32
    const amount = view.getBigUint64(offset, true)
    offset += 8
    outcomes.push({ mint, account, amount })
  }

  const deadline = view.getBigInt64(offset, true)
  offset += 8

  const salt = bytes.slice(offset, offset + 32)
  if (salt.length !== 32) throw new Error("MalformedHeader: truncated salt")
  offset += 32

  const executor = readPubkey(bytes, offset)
  offset += 32

  if (offset !== bytes.length) {
    throw new Error(
      `MalformedHeader: expected ${offset} bytes consumed, got ${bytes.length} total`
    )
  }

  return {
    user,
    srcMint,
    amountIn,
    outcomes,
    deadline,
    salt,
    executor,
  }
}

function writeOptionPubkey(
  buf: Uint8Array,
  offset: number,
  pk: PublicKey | null
): number {
  if (pk) {
    buf[offset++] = 1
    buf.set(pk.toBytes(), offset)
    offset += 32
  } else {
    buf[offset++] = 0
  }
  return offset
}

function readPubkey(bytes: Uint8Array, offset: number): PublicKey {
  if (offset + 32 > bytes.length) {
    throw new Error("MalformedHeader: truncated pubkey")
  }
  return new PublicKey(bytes.slice(offset, offset + 32))
}

function readOptionPubkey(
  bytes: Uint8Array,
  offset: number
): [PublicKey | null, number] {
  if (offset >= bytes.length) {
    throw new Error("MalformedHeader: truncated option tag")
  }
  const tag = bytes[offset++]!
  if (tag === 0) return [null, offset]
  if (tag === 1) {
    const pk = readPubkey(bytes, offset)
    return [pk, offset + 32]
  }
  throw new Error(`MalformedHeader: invalid option tag ${tag}`)
}
