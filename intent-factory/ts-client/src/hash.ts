import { sha256 } from "@noble/hashes/sha256"

/**
 * Compute the intent hash from canonical header bytes only.
 *
 * The PDA is intent-bound (not route-bound), matching the EVM/Catapultar
 * outcome model. Route calls and remaining accounts are late-bound
 * execution inputs validated by runtime outcome checks.
 *
 * ```
 * intent_hash = sha256(header_bytes)
 * ```
 */
export function computeIntentHash(headerBytes: Uint8Array): Uint8Array {
  return sha256(headerBytes)
}
