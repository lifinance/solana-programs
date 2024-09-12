"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const globals_1 = require("@jest/globals");
const _1 = require(".");
const web3_js_1 = require("@solana/web3.js");
const instructionData_1 = require("./instructionData");
const bs58_1 = __importDefault(require("bs58"));
(0, globals_1.test)("decode trackV1 instruction", async () => {
    const trackingInstructionData = (0, instructionData_1.deserializeBase58InstructionData)("1An6UebxCZV");
    (0, globals_1.expect)(trackingInstructionData).toEqual({
        TrackV1: { transaction_id: Array.from([1, 2, 3, 4, 5, 6, 7, 0]) },
    });
});
(0, globals_1.test)("build and decode trackV1 instruction", async () => {
    const trackingInstruction = _1.TrackV1.buildInstruction(new web3_js_1.PublicKey("8pt8kirWXwkMwRCkGrRHJYR7R8JKcYzdzEyWqfGj1FFa"), new Uint8Array([1, 2, 3, 4, 5, 6, 7, 0]), 600n);
    const trackingInstructionDataBase58 = bs58_1.default.encode(trackingInstruction.data);
    const decodedTrackingInstructionData = (0, instructionData_1.deserializeBase58InstructionData)(trackingInstructionDataBase58);
    (0, globals_1.expect)(decodedTrackingInstructionData).toEqual({
        TrackV1: { transaction_id: Array.from([1, 2, 3, 4, 5, 6, 7, 0]) },
    });
});
(0, globals_1.test)("reject invalid transaction id length", async () => {
    (0, globals_1.expect)(() => _1.TrackV1.buildInstruction(new web3_js_1.PublicKey("8pt8kirWXwkMwRCkGrRHJYR7R8JKcYzdzEyWqfGj1FFa"), new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0]), 600n)).toThrow();
});
