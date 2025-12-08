import { PublicKey } from "@solana/web3.js"

export * as Instructions from "./instructions/index.js"
export * from "./instructionData.js"

/**
 * Router program ID for localnet and devnet
 */
export const PROGRAM_ID_DEVNET = new PublicKey(
  "8WSaKaWhWQaTMfCEV8eLP6q4h9AyzQykPKbNZ3jZT5Ze"
)

/**
 * Router program ID on mainnet
 * Update after mainnet deployment
 */
export const PROGRAM_ID = new PublicKey(
  "8WSaKaWhWQaTMfCEV8eLP6q4h9AyzQykPKbNZ3jZT5Ze"
)
