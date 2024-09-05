import { Buffer } from "buffer";
import { borshDeserialize, BorshSchema, borshSerialize } from "borsher";
const Schema = BorshSchema.Enum({
    TrackV1: BorshSchema.Struct({
        transaction_id: BorshSchema.Array(BorshSchema.u8, 8),
    }),
});
export function serializeTrackingInstructionData(data) {
    return Buffer.from(borshSerialize(Schema, data));
}
export function deserializeTrackingInstructionData(data) {
    return borshDeserialize(Schema, data);
}
