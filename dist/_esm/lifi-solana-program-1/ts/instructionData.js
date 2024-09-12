import { Buffer } from "buffer";
import { borshDeserialize, BorshSchema, borshSerialize } from "borsher";
import bs58 from "bs58";
const Schema = BorshSchema.Enum({
    TrackV1: BorshSchema.Struct({
        transaction_id: BorshSchema.Array(BorshSchema.u8, 8),
    }),
});
/**
 * Serializes the given instruction data to a binary buffer using the __borsh__ serialization system.
 *
 * @param data The instruction data to serialize.
 * @returns The serialized data.
 */
export function serializeInstructionData(data) {
    return Buffer.from(borshSerialize(Schema, data));
}
/**
 * Deserializes the given binary instruction data to an instruction data object
 * using the __borsh__ serialization system.
 *
 * @param binaryInstructionData The binary instruction data to deserialize.
 * @returns The deserialized instruction data.
 */
export function deserializeBinaryInstructionData(binaryInstructionData) {
    return borshDeserialize(Schema, binaryInstructionData);
}
/**
 * Deserializes the given base58-encoded instruction data to an instruction data object
 * using the __borsh__ serialization system.
 *
 * @param base58InstructionData The base58-encoded instruction data to deserialize.
 * @returns The deserialized instruction data.
 */
export function deserializeBase58InstructionData(base58InstructionData) {
    return deserializeBinaryInstructionData(bs58.decode(base58InstructionData));
}
