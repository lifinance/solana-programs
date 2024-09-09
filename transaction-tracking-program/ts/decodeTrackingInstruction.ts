import {deserializeTrackingInstructionData} from "./trackingInstructionData";
import bs58 from "bs58";
import {TrackingInstructionData} from "../index";

export async function decodeTrackingInstruction(
    base58InstructionData: string
): Promise<TrackingInstructionData> {
    return decodeTrackingInstructionFromBytes(bs58.decode(base58InstructionData))
}

export async function decodeTrackingInstructionFromBytes(
    instructionDataRawBytes: Uint8Array
): Promise<TrackingInstructionData> {
    return deserializeTrackingInstructionData(instructionDataRawBytes);
}