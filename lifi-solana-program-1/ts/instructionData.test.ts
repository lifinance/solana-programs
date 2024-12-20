import { test, expect } from "vitest";
import { PublicKey } from "@solana/web3.js";
import bs58 from "bs58";
import {
  Instructions,
  PROGRAM_ADDRESS,
  deserializeBase58InstructionData,
} from "./index.js";

test("decode trackV1 instruction", async () => {
  const trackingInstructionData =
    deserializeBase58InstructionData("1An6UebxCZV");

  expect(trackingInstructionData).toEqual({
    TrackV1: { transaction_id: Array.from([1, 2, 3, 4, 5, 6, 7, 0]) },
  });
});

test("build and decode trackV1 instruction", async () => {
  const trackingInstruction = Instructions.TrackV1.buildInstruction(
    PROGRAM_ADDRESS,
    new Uint8Array([1, 2, 3, 4, 5, 6, 7, 0]),
    600n,
  );
  const trackingInstructionDataBase58 = bs58.encode(trackingInstruction.data);
  const decodedTrackingInstructionData = deserializeBase58InstructionData(
    trackingInstructionDataBase58,
  );

  expect(decodedTrackingInstructionData).toEqual({
    TrackV1: { transaction_id: Array.from([1, 2, 3, 4, 5, 6, 7, 0]) },
  });
});

test("reject invalid transaction id length", async () => {
  expect(() =>
    Instructions.TrackV1.buildInstruction(
      new PublicKey("8pt8kirWXwkMwRCkGrRHJYR7R8JKcYzdzEyWqfGj1FFa"),
      new Uint8Array([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0]),
      600n,
    ),
  ).toThrow();
});
