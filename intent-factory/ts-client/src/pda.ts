import { PublicKey } from "@solana/web3.js"
import { getAssociatedTokenAddressSync } from "@solana/spl-token"

import { computeIntentHash } from "./hash.js"

/**
 * Derive the intent PDA and bump from canonical header bytes only.
 * Seeds: `["intent", sha256(header_bytes)]`.
 *
 * Route calls and tail accounts are late-bound execution inputs and
 * do not affect the deposit address.
 */
export function deriveIntentPda(
  headerBytes: Uint8Array,
  programId: PublicKey
): [PublicKey, number] {
  const intentHash = computeIntentHash(headerBytes)
  return PublicKey.findProgramAddressSync(
    [Buffer.from("intent"), intentHash],
    programId
  )
}

/**
 * Derive the canonical source ATA: `ATA(srcMint, intentPda)`.
 */
export function deriveSourceAta(
  intentPda: PublicKey,
  srcMint: PublicKey
): PublicKey {
  return getAssociatedTokenAddressSync(srcMint, intentPda, true)
}
