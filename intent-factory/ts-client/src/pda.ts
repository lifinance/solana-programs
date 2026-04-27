import { PublicKey } from "@solana/web3.js"
import { getAssociatedTokenAddressSync } from "@solana/spl-token"

import { computeIntentHash } from "./hash.js"

/**
 * Derive the intent PDA and bump from header bytes, calls bytes, and
 * tail pubkeys. Seeds: `["intent", intent_hash]`.
 */
export function deriveIntentPda(
  headerBytes: Uint8Array,
  callsBytes: Uint8Array,
  tailPubkeys: PublicKey[],
  programId: PublicKey
): [PublicKey, number] {
  const intentHash = computeIntentHash(headerBytes, callsBytes, tailPubkeys)
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

/**
 * Derive the canonical receiver token ATA: `ATA(outMint, receiver)`.
 * Only used when `outMint` is not null.
 */
export function deriveReceiverToken(
  receiver: PublicKey,
  outMint: PublicKey
): PublicKey {
  return getAssociatedTokenAddressSync(outMint, receiver, true)
}
