"use strict";
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.decodeTrackingInstruction = decodeTrackingInstruction;
exports.decodeTrackingInstructionFromBytes = decodeTrackingInstructionFromBytes;
const trackingInstructionData_1 = require("./trackingInstructionData");
const bs58_1 = __importDefault(require("bs58"));
function decodeTrackingInstruction(base58InstructionData) {
    return __awaiter(this, void 0, void 0, function* () {
        return decodeTrackingInstructionFromBytes(bs58_1.default.decode(base58InstructionData));
    });
}
function decodeTrackingInstructionFromBytes(instructionDataRawBytes) {
    return __awaiter(this, void 0, void 0, function* () {
        return (0, trackingInstructionData_1.deserializeTrackingInstructionData)(instructionDataRawBytes);
    });
}
