var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
import { deserializeTrackingInstructionData } from "./trackingInstructionData";
import bs58 from "bs58";
export function decodeTrackingInstruction(base58InstructionData) {
    return __awaiter(this, void 0, void 0, function* () {
        return decodeTrackingInstructionFromBytes(bs58.decode(base58InstructionData));
    });
}
export function decodeTrackingInstructionFromBytes(instructionDataRawBytes) {
    return __awaiter(this, void 0, void 0, function* () {
        return deserializeTrackingInstructionData(instructionDataRawBytes);
    });
}
