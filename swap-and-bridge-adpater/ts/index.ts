import { PublicKey } from "@solana/web3.js"

export * as Instructions from "./instructions/index.js"
export * from "./instructionData.js"

/**
 * Router program ID for localnet and devnet
 */
export const PROGRAM_ID_DEVNET = new PublicKey("A8BUtjHueKntgnFD1ZQyPgBPrLd6hYsxmvxS74c9Y8cd")

/**
 * Router program ID on mainnet
 * Update after mainnet deployment
 */
export const PROGRAM_ID_MAINNET = new PublicKey("A8BUtjHueKntgnFD1ZQyPgBPrLd6hYsxmvxS74c9Y8cd")
