import { Buffer } from "buffer";
import { BorshSchema, borshSerialize } from "borsher";
const Schema = BorshSchema.Enum({
    TrackV1: BorshSchema.Struct({
        transaction_id: BorshSchema.Array(BorshSchema.u8, 9),
    }),
});
export function serializeTrackingInstructionData(data) {
    return Buffer.from(borshSerialize(Schema, data));
}
