import { Buffer } from "buffer";
import { borshDeserialize, BorshSchema, borshSerialize } from "borsher";
import bs58 from "bs58";
const Schema = BorshSchema.Enum({
    TrackV1: BorshSchema.Struct({
        transaction_id: BorshSchema.Array(BorshSchema.u8, 8),
    }),
});
export function serializeInstructionData(data) {
    return Buffer.from(borshSerialize(Schema, data));
}
export function deserializeBinaryInstructionData(binaryInstructionData) {
    return borshDeserialize(Schema, binaryInstructionData);
}
export function deserializeBase58InstructionData(base58InstructionData) {
    return deserializeBinaryInstructionData(bs58.decode(base58InstructionData));
}
