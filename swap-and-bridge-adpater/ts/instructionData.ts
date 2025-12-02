import { borshSerialize, BorshSchema } from "borsher"

/**
 * Parameters for the swap_and_bridge instruction
 */
export type SwapAndBridgeParams = {
  /** Unique seed per route (8 bytes), used to derive vault authority PDA */
  routeSeed: Uint8Array
  /** Minimum amount expected after swap (slippage guard) */
  minAmount: bigint
  /** Adapter selector (0 = Unit, others in the future) */
  adapterId: number
  /** Opaque adapter-specific payload, Borsh-encoded */
  adapterPayload: Uint8Array
}

/**
 * Instruction data enum for the router program
 */
export type InstructionData = {
  SwapAndBridge: SwapAndBridgeParams
}

/**
 * Anchor discriminator for swap_and_bridge instruction
 * Pre-computed: sha256("global:swap_and_bridge")[0..8]
 */
const SWAP_AND_BRIDGE_DISCRIMINATOR = new Uint8Array([204, 63, 169, 171, 186, 125, 86, 159])

/**
 * Borsh schema for swap_and_bridge params
 */
const ParamsSchema: BorshSchema = BorshSchema.Struct({
  routeSeed: BorshSchema.Array(BorshSchema.u8, 8),
  minAmount: BorshSchema.u64,
  adapterId: BorshSchema.u8,
  adapterPayload: BorshSchema.Vec(BorshSchema.u8),
})

/**
 * Serializes swap_and_bridge instruction parameters to binary format
 *
 * @param data - The instruction data
 * @returns Serialized instruction data with Anchor discriminator
 */
export function serializeInstructionData(data: InstructionData): Uint8Array {
  if (data.SwapAndBridge.routeSeed.length !== 8) {
    throw new Error(`routeSeed must be 8 bytes, got ${data.SwapAndBridge.routeSeed.length}`)
  }

  // Serialize params
  const params = borshSerialize(ParamsSchema, data.SwapAndBridge)

  // Prepend Anchor discriminator
  const result = new Uint8Array(8 + params.length)
  result.set(SWAP_AND_BRIDGE_DISCRIMINATOR, 0)
  result.set(new Uint8Array(params), 8)

  return result
}
