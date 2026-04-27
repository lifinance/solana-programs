import { sha256 } from "@noble/hashes/sha256"
import { PublicKey } from "@solana/web3.js"

/**
 * ```
 * acc_buf      = u8(tail.len()) || concat(pubkey for pubkey in tail)
 * calls_digest = sha256(acc_buf || calls_bytes)
 * ```
 */
export function computeCallsDigest(
  callsBytes: Uint8Array,
  tailPubkeys: PublicKey[]
): Uint8Array {
  const accBuf = new Uint8Array(1 + tailPubkeys.length * 32)
  accBuf[0] = tailPubkeys.length
  let offset = 1
  for (const pk of tailPubkeys) {
    accBuf.set(pk.toBytes(), offset)
    offset += 32
  }

  const digestInput = new Uint8Array(accBuf.length + callsBytes.length)
  digestInput.set(accBuf)
  digestInput.set(callsBytes, accBuf.length)

  return sha256(digestInput)
}

/**
 * ```
 * intent_hash = sha256(header_bytes || calls_digest)
 * ```
 */
export function computeIntentHash(
  headerBytes: Uint8Array,
  callsBytes: Uint8Array,
  tailPubkeys: PublicKey[]
): Uint8Array {
  const callsDigest = computeCallsDigest(callsBytes, tailPubkeys)

  const hashInput = new Uint8Array(headerBytes.length + 32)
  hashInput.set(headerBytes)
  hashInput.set(callsDigest, headerBytes.length)

  return sha256(hashInput)
}
