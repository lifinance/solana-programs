"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.serializeTrackingInstructionData = serializeTrackingInstructionData;
exports.deserializeTrackingInstructionData = deserializeTrackingInstructionData;
const buffer_1 = require("buffer");
const borsher_1 = require("borsher");
const Schema = borsher_1.BorshSchema.Enum({
    TrackV1: borsher_1.BorshSchema.Struct({
        transaction_id: borsher_1.BorshSchema.Array(borsher_1.BorshSchema.u8, 8),
    }),
});
function serializeTrackingInstructionData(data) {
    return buffer_1.Buffer.from((0, borsher_1.borshSerialize)(Schema, data));
}
function deserializeTrackingInstructionData(data) {
    return (0, borsher_1.borshDeserialize)(Schema, data);
}
