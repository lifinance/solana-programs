use anchor_lang::prelude::*;
use anchor_lang::solana_program::{instruction::Instruction, keccak, program::{invoke, invoke_signed}};
use anchor_spl::token::spl_token;

use crate::{ErrorCode, SwapAndBridge, SwapAndBridgeParams};

// ------------------------------------------------------------
// ACROSS ADAPTER
// ------------------------------------------------------------

/// Across SVM Spoke Program ID (mainnet)
pub const ACROSS_PROGRAM_ID: Pubkey = pubkey!("DLv3NggMiSaef97YCkew5xKUHDh13tVGZ7tydt3ZeAru");

/// Across state seed for mainnet (0)
pub const ACROSS_STATE_SEED: u64 = 0;

/// Base for output amount multiplier (1e18 to match EVM)
pub const MULTIPLIER_BASE: u128 = 1_000_000_000_000_000_000;

/// Payload structure for Across adapter
/// Maps directly to backend data from generateAcrossInstructionsSolana
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct AcrossAdapterPayload {
    /// Recipient on destination chain (evmAddressToSolanaPublicKey(toAddress))
    pub recipient: Pubkey,
    /// Output token on destination (evmAddressToSolanaPublicKey(toToken.address))
    pub output_token: Pubkey,
    /// Output amount multiplier (scaled by 1e18, accounts for fee ratio and decimal differences)
    /// Formula: outputAmount = (inputAmount * outputAmountMultiplier) / MULTIPLIER_BASE
    pub output_amount_multiplier: u128,
    /// Destination chain ID
    pub destination_chain_id: u64,
    /// Exclusive relayer (from relayer lookup, or Pubkey::default())
    pub exclusive_relayer: Pubkey,
    /// Quote timestamp from Across API
    pub quote_timestamp: u32,
    /// Fill deadline timestamp
    pub fill_deadline: u32,
    /// Exclusivity parameter (deadline offset or absolute timestamp)
    pub exclusivity_parameter: u32,
    /// Optional message for cross-chain calls (usually empty)
    pub message: Vec<u8>,
}

/// Seed data structure for computing delegate PDA seed hash
/// Must match Across's DepositSeedData serialization exactly
#[derive(AnchorSerialize)]
struct DepositSeedData<'a> {
    depositor: Pubkey,
    recipient: Pubkey,
    input_token: Pubkey,
    output_token: Pubkey,
    input_amount: u64,
    output_amount: [u8; 32],
    destination_chain_id: u64,
    exclusive_relayer: Pubkey,
    quote_timestamp: u32,
    fill_deadline: u32,
    exclusivity_parameter: u32,
    message: &'a Vec<u8>,
}

/// Compute seed hash using keccak256 (matches Across's derive_seed_hash)
fn derive_seed_hash<T: AnchorSerialize>(seed: &T) -> [u8; 32] {
    let mut data = Vec::new();
    AnchorSerialize::serialize(seed, &mut data).unwrap();
    keccak::hash(&data).to_bytes()
}

/// Convert u128 to [u8; 32] big-endian (for EVM-compatible output amounts)
/// The u128 value occupies the lower 16 bytes, upper 16 bytes are zero-padded
fn u128_to_bytes32_be(value: u128) -> [u8; 32] {
    let mut result = [0u8; 32];
    result[16..].copy_from_slice(&value.to_be_bytes());
    result
}

/// Bridge tokens via Across adapter
///
/// This function:
/// 1. Decodes the Across adapter payload
/// 2. Computes the delegate PDA seed hash from deposit params
/// 3. Derives the delegate PDA
/// 4. Approves the delegate to spend from our vault
/// 5. CPIs into Across's deposit instruction
///
/// # Accounts (from remaining_accounts)
/// - Position 0 (7): across_state - Across State PDA
/// - Position 1 (8): across_vault - Across vault ATA (state-owned)
/// - Position 2 (9): across_program - Across svm-spoke program
pub fn bridge_via_across<'info>(
    ctx: &anchor_lang::context::Context<'_, '_, '_, 'info, SwapAndBridge<'info>>,
    params: &SwapAndBridgeParams,
    amount: u64,
    vault_bump: u8,
) -> Result<()> {
    // 1) Decode adapter payload
    let payload: AcrossAdapterPayload = AcrossAdapterPayload::try_from_slice(&params.adapter_payload)
        .map_err(|_| ErrorCode::InvalidAdapterPayload)?;

    // 2) Access Across accounts from remaining_accounts
    let remaining = &ctx.remaining_accounts;
    require!(remaining.len() >= 3, ErrorCode::InsufficientAccounts);

    let across_state = &remaining[0];
    let across_vault = &remaining[1];
    let across_program = &remaining[2];

    // 3) Validate Across program ID
    require_keys_eq!(
        across_program.key(),
        ACROSS_PROGRAM_ID,
        ErrorCode::InvalidAcrossProgram
    );

    // 4) Compute output amount from multiplier
    // Formula: outputAmount = (inputAmount * outputAmountMultiplier) / MULTIPLIER_BASE
    let computed_output: u128 = (amount as u128)
        .checked_mul(payload.output_amount_multiplier)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(MULTIPLIER_BASE)
        .ok_or(ErrorCode::MathOverflow)?;
    let output_amount = u128_to_bytes32_be(computed_output);

    // 5) Build DepositSeedData with actual vault balance as input_amount
    let depositor = ctx.accounts.vault_authority.key();
    let input_token = ctx.accounts.mint.key();
    let message_vec = payload.message.clone();

    let seed_data = DepositSeedData {
        depositor,
        recipient: payload.recipient,
        input_token,
        output_token: payload.output_token,
        input_amount: amount,
        output_amount,
        destination_chain_id: payload.destination_chain_id,
        exclusive_relayer: payload.exclusive_relayer,
        quote_timestamp: payload.quote_timestamp,
        fill_deadline: payload.fill_deadline,
        exclusivity_parameter: payload.exclusivity_parameter,
        message: &message_vec,
    };

    // 6) Compute seed hash and derive delegate PDA
    let seed_hash = derive_seed_hash(&seed_data);
    let (delegate_pda, _delegate_bump) =
        Pubkey::find_program_address(&[b"delegate", &seed_hash], &ACROSS_PROGRAM_ID);

    // 7) Approve delegate PDA to spend from our vault
    let mint_key = ctx.accounts.mint.key();
    let vault_seeds: &[&[u8]] = &[
        b"vault",
        params.route_seed.as_ref(),
        mint_key.as_ref(),
        &[vault_bump],
    ];
    let signer_seeds = &[vault_seeds];

    // Use raw approve instruction to set delegate to the derived PDA
    let approve_ix = spl_token::instruction::approve(
        &ctx.accounts.token_program.key(),
        &ctx.accounts.intermediate_vault.key(),
        &delegate_pda,
        &ctx.accounts.vault_authority.key(),
        &[],
        amount,
    )
    .map_err(|_| ErrorCode::ApproveError)?;

    invoke_signed(
        &approve_ix,
        &[
            ctx.accounts.intermediate_vault.to_account_info(),
            ctx.accounts.vault_authority.to_account_info(),
        ],
        signer_seeds,
    )?;

    // 8) Build Across deposit instruction
    // Instruction discriminator for "deposit" (first 8 bytes of sha256("global:deposit"))
    let deposit_discriminator: [u8; 8] = [242, 35, 198, 137, 82, 225, 242, 182];

    // Serialize deposit instruction data
    let mut deposit_data = Vec::new();
    deposit_data.extend_from_slice(&deposit_discriminator);
    AnchorSerialize::serialize(&depositor, &mut deposit_data)?;
    AnchorSerialize::serialize(&payload.recipient, &mut deposit_data)?;
    AnchorSerialize::serialize(&input_token, &mut deposit_data)?;
    AnchorSerialize::serialize(&payload.output_token, &mut deposit_data)?;
    AnchorSerialize::serialize(&amount, &mut deposit_data)?;
    AnchorSerialize::serialize(&output_amount, &mut deposit_data)?;
    AnchorSerialize::serialize(&payload.destination_chain_id, &mut deposit_data)?;
    AnchorSerialize::serialize(&payload.exclusive_relayer, &mut deposit_data)?;
    AnchorSerialize::serialize(&payload.quote_timestamp, &mut deposit_data)?;
    AnchorSerialize::serialize(&payload.fill_deadline, &mut deposit_data)?;
    AnchorSerialize::serialize(&payload.exclusivity_parameter, &mut deposit_data)?;
    AnchorSerialize::serialize(&payload.message, &mut deposit_data)?;

    // 9) Build account metas for Across deposit CPI
    let deposit_accounts = vec![
        AccountMeta::new(ctx.accounts.payer.key(), true),  // signer
        AccountMeta::new(across_state.key(), false),       // state
        AccountMeta::new_readonly(delegate_pda, false),    // delegate
        AccountMeta::new(ctx.accounts.intermediate_vault.key(), false), // depositor_token_account
        AccountMeta::new(across_vault.key(), false),       // vault
        AccountMeta::new_readonly(ctx.accounts.mint.key(), false), // mint
        AccountMeta::new_readonly(ctx.accounts.token_program.key(), false), // token_program
        AccountMeta::new_readonly(ctx.accounts.associated_token_program.key(), false), // associated_token_program
        AccountMeta::new_readonly(ctx.accounts.system_program.key(), false), // system_program
    ];

    let deposit_ix = Instruction {
        program_id: ACROSS_PROGRAM_ID,
        accounts: deposit_accounts,
        data: deposit_data,
    };

    // 10) Invoke Across deposit CPI
    invoke(
        &deposit_ix,
        &[
            ctx.accounts.payer.to_account_info(),
            across_state.clone(),
            ctx.accounts.intermediate_vault.to_account_info(),
            across_vault.clone(),
            ctx.accounts.mint.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.associated_token_program.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        ],
    )?;

    Ok(())
}

