import { PublicKey } from "@solana/web3.js"
import { borshSerialize, BorshSchema } from "borsher"

/**
 * Payload structure for Unit adapter
 */
export type UnitAdapterPayload = {
  /** Unit deposit wallet (from getUnitDepositAddress) */
  unitDepositWallet: PublicKey
  /** Future extension: 0 = SPL/wSOL, 1 = native SOL, etc. */
  isNative: number
}

/**
 * Borsh schema for Unit adapter payload
 */
const UnitAdapterPayloadSchema = BorshSchema.Struct({
  unitDepositWallet: BorshSchema.Array(BorshSchema.u8, 32), // PublicKey is 32 bytes
  isNative: BorshSchema.u8,
})

/**
 * Serializes Unit adapter payload to binary format
 *
 * @param payload - The Unit adapter payload
 * @returns Serialized payload bytes
 */
export function serializeUnitPayload(payload: UnitAdapterPayload): Uint8Array {
  return borshSerialize(UnitAdapterPayloadSchema, {
    unitDepositWallet: payload.unitDepositWallet.toBytes(),
    isNative: payload.isNative,
  })
}
