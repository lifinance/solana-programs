import { CompiledInstruction } from "@solana/web3.js";
import { TrackingInstructionData } from "../index";
export declare function decodeTrackingInstruction(instruction: CompiledInstruction): Promise<TrackingInstructionData>;
