import { Buffer } from "buffer";
/**
 * The input data given to the solana program.
 * This enum determines which instruction gets executed and provides the data for it.
 *
 * For now, only the `TrackV1` instruction is implemented.
 */
export type InstructionData = {
  TrackV1: {
    transaction_id: Uint8Array;
  };
};
/**
 * Serializes the given instruction data to a binary buffer using the __borsh__ serialization system.
 *
 * @param data The instruction data to serialize.
 * @returns The serialized data.
 */
export declare function serializeInstructionData(data: InstructionData): Buffer;
/**
 * Deserializes the given binary instruction data to an instruction data object
 * using the __borsh__ serialization system.
 *
 * @param binaryInstructionData The binary instruction data to deserialize.
 * @returns The deserialized instruction data.
 */
export declare function deserializeBinaryInstructionData(
  binaryInstructionData: Uint8Array,
): InstructionData;
/**
 * Deserializes the given base58-encoded instruction data to an instruction data object
 * using the __borsh__ serialization system.
 *
 * @param base58InstructionData The base58-encoded instruction data to deserialize.
 * @returns The deserialized instruction data.
 */
export declare function deserializeBase58InstructionData(
  base58InstructionData: string,
): InstructionData;
