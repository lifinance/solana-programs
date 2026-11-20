export const MAX_CALLS = 8
export const MAX_ACCOUNTS_PER_CALL = 64
export const MAX_DATA_LEN = 1024
export const NAMED_PREFIX = 2

export interface CallSpecLike {
  programIx: number
  accounts: number[]
  isWritable: boolean[]
  isSigner: boolean[]
  data: Uint8Array
}

export function encodeCalls(calls: CallSpecLike[]): Uint8Array {
  if (calls.length > MAX_CALLS) {
    throw new Error(`WireError: num_calls ${calls.length} > ${MAX_CALLS}`)
  }

  const parts: Uint8Array[] = []
  parts.push(new Uint8Array([calls.length]))

  for (const call of calls) {
    if (call.programIx < NAMED_PREFIX) {
      throw new Error(
        `WireError: program_ix ${call.programIx} < NAMED_PREFIX`
      )
    }
    const accCount = call.accounts.length
    if (accCount > MAX_ACCOUNTS_PER_CALL) {
      throw new Error(
        `WireError: acc_count ${accCount} > ${MAX_ACCOUNTS_PER_CALL}`
      )
    }
    if (call.data.length > MAX_DATA_LEN) {
      throw new Error(
        `WireError: data_len ${call.data.length} > ${MAX_DATA_LEN}`
      )
    }

    parts.push(new Uint8Array([call.programIx]))
    parts.push(new Uint8Array([accCount]))
    parts.push(new Uint8Array(call.accounts))

    const bitmapLen = Math.ceil((accCount * 2) / 8)
    const bitmap = new Uint8Array(bitmapLen)
    for (let i = 0; i < accCount; i++) {
      const bitOffset = i * 2
      const byteIx = Math.floor(bitOffset / 8)
      const bitIx = bitOffset % 8
      if (call.isWritable[i]) {
        bitmap[byteIx]! |= 1 << bitIx
      }
      if (call.isSigner[i]) {
        bitmap[byteIx]! |= 1 << (bitIx + 1)
      }
    }
    parts.push(bitmap)

    const dataLenBuf = new Uint8Array(2)
    new DataView(dataLenBuf.buffer).setUint16(0, call.data.length, true)
    parts.push(dataLenBuf)
    parts.push(call.data)
  }

  const totalLen = parts.reduce((s, p) => s + p.length, 0)
  const result = new Uint8Array(totalLen)
  let offset = 0
  for (const p of parts) {
    result.set(p, offset)
    offset += p.length
  }
  return result
}

export function decodeCalls(
  buf: Uint8Array,
  tailLen: number
): CallSpecLike[] {
  if (buf.length === 0) {
    throw new Error("WireError: empty buffer")
  }

  const virtualListLen = NAMED_PREFIX + tailLen
  const numCalls = buf[0]!
  if (numCalls > MAX_CALLS) {
    throw new Error(`WireError: num_calls ${numCalls} > ${MAX_CALLS}`)
  }

  let cursor = 1
  const calls: CallSpecLike[] = []

  for (let c = 0; c < numCalls; c++) {
    const programIx = buf[cursor++]!
    if (programIx < NAMED_PREFIX) {
      throw new Error(
        `WireError: program_ix ${programIx} < NAMED_PREFIX`
      )
    }
    if (programIx >= virtualListLen) {
      throw new Error(`WireError: program_ix ${programIx} out of bounds`)
    }

    const accCount = buf[cursor++]!
    if (accCount > MAX_ACCOUNTS_PER_CALL) {
      throw new Error(
        `WireError: acc_count ${accCount} > ${MAX_ACCOUNTS_PER_CALL}`
      )
    }

    const accounts: number[] = []
    for (let i = 0; i < accCount; i++) {
      const ix = buf[cursor++]!
      if (ix >= virtualListLen) {
        throw new Error(`WireError: account index ${ix} out of bounds`)
      }
      accounts.push(ix)
    }

    const bitmapLen = Math.ceil((accCount * 2) / 8)
    const flagsBitmap = buf.slice(cursor, cursor + bitmapLen)
    cursor += bitmapLen

    if (accCount > 0) {
      validateTrailingBitmapBits(flagsBitmap, accCount)
    }

    const isWritable: boolean[] = []
    const isSigner: boolean[] = []
    for (let i = 0; i < accCount; i++) {
      const [w, s] = flagFor(flagsBitmap, i)
      isWritable.push(w)
      isSigner.push(s)
    }

    if (cursor + 2 > buf.length) {
      throw new Error("WireError: truncated data_len")
    }
    const dataLen = new DataView(
      buf.buffer,
      buf.byteOffset + cursor,
      2
    ).getUint16(0, true)
    cursor += 2

    if (dataLen > MAX_DATA_LEN) {
      throw new Error(`WireError: data_len ${dataLen} > ${MAX_DATA_LEN}`)
    }

    const data = buf.slice(cursor, cursor + dataLen)
    if (data.length !== dataLen) {
      throw new Error("WireError: truncated data")
    }
    cursor += dataLen

    calls.push({ programIx, accounts, isWritable, isSigner, data })
  }

  if (cursor !== buf.length) {
    throw new Error(
      `WireError: trailing bytes (consumed ${cursor}, total ${buf.length})`
    )
  }

  return calls
}

export function flagFor(
  bitmap: Uint8Array,
  i: number
): [boolean, boolean] {
  const bitOffset = i * 2
  const byteIx = Math.floor(bitOffset / 8)
  const bitIx = bitOffset % 8
  if (byteIx >= bitmap.length) return [false, false]
  const byte = bitmap[byteIx]!
  const isWritable = ((byte >> bitIx) & 1) === 1
  const isSigner = ((byte >> (bitIx + 1)) & 1) === 1
  return [isWritable, isSigner]
}

function validateTrailingBitmapBits(
  bitmap: Uint8Array,
  accCount: number
): void {
  const usedBits = accCount * 2
  const usedInLast = usedBits % 8
  if (usedInLast === 0) return
  const lastByte = bitmap[bitmap.length - 1]!
  const mask = ~((1 << usedInLast) - 1) & 0xff
  if ((lastByte & mask) !== 0) {
    throw new Error("WireError: non-zero trailing bitmap bits")
  }
}
