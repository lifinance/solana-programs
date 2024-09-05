import {CompiledInstruction} from "@solana/web3.js";
import {deserializeTrackingInstructionData} from "./trackingInstructionData";
import bs58 from "bs58";
import {TrackingInstructionData} from "../index";

export async function decodeTrackingInstruction(
    instruction: CompiledInstruction
): Promise<TrackingInstructionData> {
    const rawData = bs58.decode(instruction.data);
    return deserializeTrackingInstructionData(new Uint8Array(rawData));
}