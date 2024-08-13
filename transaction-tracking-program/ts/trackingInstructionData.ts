import {Buffer} from "buffer";
import {BorshSchema, borshSerialize} from "borsher";

export type TrackingInstructionData =
    | {
    TrackV1: {
        transaction_id: Uint8Array;
    };
};
const Schema: BorshSchema = BorshSchema.Enum({
    TrackV1: BorshSchema.Struct({
        transaction_id: BorshSchema.Array(BorshSchema.u8, 9),
    }),
})

export function serializeTrackingInstructionData(data: TrackingInstructionData): Buffer {
    return Buffer.from(
        borshSerialize(Schema, data)
    )
}

