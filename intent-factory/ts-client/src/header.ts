import { PublicKey } from "@solana/web3.js"

export const MAX_FEES = 4

export interface IntentHeader {
  user: PublicKey
  srcMint: PublicKey | null
  amountIn: bigint
  outMint: PublicKey | null
  receiver: PublicKey
  minAmountOut: bigint
  feeRecipients: Array<{ pubkey: PublicKey; amount: bigint }>
  deadline: bigint
  salt: Uint8Array
  executor: PublicKey | null
}

export function encodeIntentHeader(header: IntentHeader): Uint8Array {
  const sorted = [...header.feeRecipients].sort((a, b) => {
    const pkCmp = compareBytes(a.pubkey.toBytes(), b.pubkey.toBytes())
    if (pkCmp !== 0) return pkCmp
    return a.amount < b.amount ? -1 : a.amount > b.amount ? 1 : 0
  })

  const feeCount = sorted.length
  const capacity = 32 + 33 + 8 + 33 + 32 + 8 + 1 + feeCount * 40 + 8 + 32 + 33
  const buf = new Uint8Array(capacity)
  const view = new DataView(buf.buffer)
  let offset = 0

  buf.set(header.user.toBytes(), offset)
  offset += 32

  offset = writeOptionPubkey(buf, offset, header.srcMint)

  view.setBigUint64(offset, header.amountIn, true)
  offset += 8

  offset = writeOptionPubkey(buf, offset, header.outMint)

  buf.set(header.receiver.toBytes(), offset)
  offset += 32

  view.setBigUint64(offset, header.minAmountOut, true)
  offset += 8

  buf[offset++] = feeCount
  for (const fee of sorted) {
    buf.set(fee.pubkey.toBytes(), offset)
    offset += 32
    view.setBigUint64(offset, fee.amount, true)
    offset += 8
  }

  view.setBigInt64(offset, header.deadline, true)
  offset += 8

  buf.set(header.salt, offset)
  offset += 32

  offset = writeOptionPubkey(buf, offset, header.executor)

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

  const [outMint, outMintEnd] = readOptionPubkey(bytes, offset)
  offset = outMintEnd

  const receiver = readPubkey(bytes, offset)
  offset += 32

  const minAmountOut = view.getBigUint64(offset, true)
  offset += 8

  const feeCount = bytes[offset++]!
  if (feeCount > MAX_FEES) {
    throw new Error(`MalformedHeader: fee_count ${feeCount} > ${MAX_FEES}`)
  }

  const feeRecipients: Array<{ pubkey: PublicKey; amount: bigint }> = []
  for (let i = 0; i < feeCount; i++) {
    const pubkey = readPubkey(bytes, offset)
    offset += 32
    const amount = view.getBigUint64(offset, true)
    offset += 8
    feeRecipients.push({ pubkey, amount })
  }

  validateFeeSort(feeRecipients)

  const deadline = view.getBigInt64(offset, true)
  offset += 8

  const salt = bytes.slice(offset, offset + 32)
  if (salt.length !== 32) throw new Error("MalformedHeader: truncated salt")
  offset += 32

  const [executor, executorEnd] = readOptionPubkey(bytes, offset)
  offset = executorEnd

  if (offset !== bytes.length) {
    throw new Error(
      `MalformedHeader: expected ${offset} bytes consumed, got ${bytes.length} total`
    )
  }

  return {
    user,
    srcMint,
    amountIn,
    outMint,
    receiver,
    minAmountOut,
    feeRecipients,
    deadline,
    salt,
    executor,
  }
}

function validateFeeSort(
  fees: Array<{ pubkey: PublicKey; amount: bigint }>
): void {
  for (let i = 0; i < fees.length - 1; i++) {
    const a = fees[i]!
    const b = fees[i + 1]!
    if (a.pubkey.equals(b.pubkey)) {
      throw new Error("MalformedHeader: duplicate fee pubkey")
    }
    if (compareBytes(a.pubkey.toBytes(), b.pubkey.toBytes()) >= 0) {
      throw new Error("MalformedHeader: fee recipients not strictly sorted")
    }
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

function compareBytes(a: Uint8Array, b: Uint8Array): number {
  const len = Math.min(a.length, b.length)
  for (let i = 0; i < len; i++) {
    if (a[i]! < b[i]!) return -1
    if (a[i]! > b[i]!) return 1
  }
  return a.length - b.length
}
