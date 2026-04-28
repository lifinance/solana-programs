import { describe, it, expect } from "vitest"
import { PublicKey } from "@solana/web3.js"
import { readFileSync } from "fs"
import { resolve } from "path"

import {
  encodeIntentHeader,
  decodeIntentHeader,
  encodeCalls,
  decodeCalls,
  computeIntentHash,
} from "../ts-client/src/index.js"
import type { IntentHeader, CallSpecLike } from "../ts-client/src/index.js"

function hexToBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2)
  for (let i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16)
  }
  return bytes
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("")
}

const fixturePath = resolve(__dirname, "fixtures/intent_hash.json")
const fixture = JSON.parse(readFileSync(fixturePath, "utf-8"))

function buildHeaderFromFixture(): IntentHeader {
  const h = fixture.header
  return {
    user: new PublicKey(hexToBytes(h.user)),
    srcMint: h.src_mint ? new PublicKey(hexToBytes(h.src_mint)) : null,
    amountIn: BigInt(h.amount_in),
    outMint: h.out_mint ? new PublicKey(hexToBytes(h.out_mint)) : null,
    receiver: new PublicKey(hexToBytes(h.receiver)),
    minAmountOut: BigInt(h.min_amount_out),
    feeRecipients: h.fee_recipients.map(
      (f: { pubkey: string; amount: number }) => ({
        pubkey: new PublicKey(hexToBytes(f.pubkey)),
        amount: BigInt(f.amount),
      })
    ),
    deadline: BigInt(h.deadline),
    salt: hexToBytes(h.salt),
    executor: new PublicKey(hexToBytes(h.executor)),
  }
}

function buildCallsFromFixture(): CallSpecLike[] {
  return fixture.calls.map(
    (c: {
      program_ix: number
      accounts: number[]
      is_writable: boolean[]
      is_signer: boolean[]
      data: string
    }) => ({
      programIx: c.program_ix,
      accounts: c.accounts,
      isWritable: c.is_writable,
      isSigner: c.is_signer,
      data: hexToBytes(c.data),
    })
  )
}

describe("Protocol Conformance", () => {
  describe("Header encoding", () => {
    it("encodeIntentHeader matches Rust fixture bytes", () => {
      const header = buildHeaderFromFixture()
      const encoded = encodeIntentHeader(header)
      expect(bytesToHex(encoded)).toBe(fixture.header_bytes)
    })

    it("decodeIntentHeader round-trips lossless", () => {
      const headerBytes = hexToBytes(fixture.header_bytes)
      const decoded = decodeIntentHeader(headerBytes)
      const reEncoded = encodeIntentHeader(decoded)
      expect(bytesToHex(reEncoded)).toBe(fixture.header_bytes)
    })

    it("decodeIntentHeader preserves field values", () => {
      const headerBytes = hexToBytes(fixture.header_bytes)
      const decoded = decodeIntentHeader(headerBytes)
      expect(decoded.amountIn).toBe(BigInt(fixture.header.amount_in))
      expect(decoded.minAmountOut).toBe(BigInt(fixture.header.min_amount_out))
      expect(decoded.deadline).toBe(BigInt(fixture.header.deadline))
      expect(decoded.feeRecipients.length).toBe(
        fixture.header.fee_recipients.length
      )
    })
  })

  describe("Calls encoding", () => {
    it("encodeCalls matches Rust fixture bytes", () => {
      const calls = buildCallsFromFixture()
      const encoded = encodeCalls(calls)
      expect(bytesToHex(encoded)).toBe(fixture.calls_bytes)
    })

    it("decodeCalls round-trips lossless", () => {
      const callsBytes = hexToBytes(fixture.calls_bytes)
      const tailLen = fixture.tail_pubkeys.length
      const decoded = decodeCalls(callsBytes, tailLen)
      const reEncoded = encodeCalls(decoded)
      expect(bytesToHex(reEncoded)).toBe(fixture.calls_bytes)
    })

    it("decodeCalls preserves call structure", () => {
      const callsBytes = hexToBytes(fixture.calls_bytes)
      const tailLen = fixture.tail_pubkeys.length
      const decoded = decodeCalls(callsBytes, tailLen)
      expect(decoded.length).toBe(fixture.calls.length)
      for (let i = 0; i < decoded.length; i++) {
        const d = decoded[i]!
        const f = fixture.calls[i]!
        expect(d.programIx).toBe(f.program_ix)
        expect(d.accounts).toEqual(f.accounts)
        expect(d.isWritable).toEqual(f.is_writable)
        expect(d.isSigner).toEqual(f.is_signer)
        expect(bytesToHex(d.data)).toBe(f.data)
      }
    })
  })

  describe("Intent hash", () => {
    it("computeIntentHash matches Rust fixture", () => {
      const headerBytes = hexToBytes(fixture.header_bytes)
      const hash = computeIntentHash(headerBytes)
      expect(bytesToHex(hash)).toBe(fixture.intent_hash)
    })

    it("hash changes when header is modified", () => {
      const header = buildHeaderFromFixture()
      header.amountIn = 999_999n
      const modifiedHeaderBytes = encodeIntentHeader(header)
      const hash = computeIntentHash(modifiedHeaderBytes)
      expect(bytesToHex(hash)).not.toBe(fixture.intent_hash)
    })

    it("hash does not change when route/tail changes", () => {
      const headerBytes = hexToBytes(fixture.header_bytes)
      const hash1 = computeIntentHash(headerBytes)
      const hash2 = computeIntentHash(headerBytes)
      expect(bytesToHex(hash1)).toBe(bytesToHex(hash2))
    })

    it("changing salt changes the hash", () => {
      const header = buildHeaderFromFixture()
      const hash1 = computeIntentHash(encodeIntentHeader(header))

      header.salt = new Uint8Array(32).fill(0xff)
      const hash2 = computeIntentHash(encodeIntentHeader(header))

      expect(bytesToHex(hash1)).not.toBe(bytesToHex(hash2))
    })
  })

  describe("Header negative fixtures", () => {
    it("rejects bad option tag", () => {
      const bytes = hexToBytes(fixture.header_bytes)
      bytes[32] = 2 // corrupt src_mint tag
      expect(() => decodeIntentHeader(bytes)).toThrow("MalformedHeader")
    })

    it("rejects fee_count > 4", () => {
      const bytes = hexToBytes(fixture.header_bytes)
      // user(32) + src_mint(33) + amount_in(8) + out_mint(33) + receiver(32) + min_amount_out(8) = 146
      bytes[146] = 5
      expect(() => decodeIntentHeader(bytes)).toThrow("MalformedHeader")
    })

    it("rejects trailing bytes", () => {
      const bytes = hexToBytes(fixture.header_bytes)
      const extended = new Uint8Array(bytes.length + 1)
      extended.set(bytes)
      expect(() => decodeIntentHeader(extended)).toThrow("MalformedHeader")
    })

    it("rejects truncated bytes", () => {
      const bytes = hexToBytes(fixture.header_bytes)
      expect(() => decodeIntentHeader(bytes.slice(0, bytes.length - 1))).toThrow(
        "MalformedHeader"
      )
    })
  })

  describe("Wire negative fixtures", () => {
    it("rejects num_calls > MAX_CALLS", () => {
      const buf = new Uint8Array([9, ...new Array(50).fill(0)])
      expect(() => decodeCalls(buf, 10)).toThrow("WireError")
    })

    it("rejects program_ix < NAMED_PREFIX", () => {
      expect(() =>
        encodeCalls([
          {
            programIx: 3,
            accounts: [0],
            isWritable: [true],
            isSigner: [false],
            data: new Uint8Array(),
          },
        ])
      ).toThrow("WireError")
    })

    it("rejects data_len > MAX_DATA_LEN", () => {
      expect(() =>
        encodeCalls([
          {
            programIx: 4,
            accounts: [0],
            isWritable: [true],
            isSigner: [false],
            data: new Uint8Array(1025),
          },
        ])
      ).toThrow("WireError")
    })

    it("rejects non-zero trailing bitmap bits", () => {
      // Manually craft wire bytes with bad bitmap
      const buf = new Uint8Array([
        1,    // num_calls = 1
        4,    // program_ix = 4
        1,    // acc_count = 1
        0,    // accounts[0] = 0
        0b11111101, // bitmap with trailing bits set
        0, 0, // data_len = 0
      ])
      expect(() => decodeCalls(buf, 10)).toThrow("WireError")
    })

    it("rejects trailing wire bytes", () => {
      const calls = buildCallsFromFixture()
      const encoded = encodeCalls(calls)
      const extended = new Uint8Array(encoded.length + 1)
      extended.set(encoded)
      extended[encoded.length] = 0xff
      expect(() =>
        decodeCalls(extended, fixture.tail_pubkeys.length)
      ).toThrow("WireError")
    })
  })
})
