"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.serializeInstructionData = serializeInstructionData;
exports.deserializeBinaryInstructionData = deserializeBinaryInstructionData;
exports.deserializeBase58InstructionData = deserializeBase58InstructionData;
const buffer_1 = require("buffer");
const borsher_1 = require("borsher");
const bs58_1 = __importDefault(require("bs58"));
const Schema = borsher_1.BorshSchema.Enum({
    TrackV1: borsher_1.BorshSchema.Struct({
        transaction_id: borsher_1.BorshSchema.Array(borsher_1.BorshSchema.u8, 8),
    }),
});
function serializeInstructionData(data) {
    return buffer_1.Buffer.from((0, borsher_1.borshSerialize)(Schema, data));
}
function deserializeBinaryInstructionData(binaryInstructionData) {
    return (0, borsher_1.borshDeserialize)(Schema, binaryInstructionData);
}
function deserializeBase58InstructionData(base58InstructionData) {
    return deserializeBinaryInstructionData(bs58_1.default.decode(base58InstructionData));
}
