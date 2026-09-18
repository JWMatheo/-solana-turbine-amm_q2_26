#![allow(clippy::result_large_err)]

use {
    amm_video::constants::{
        CONFIG_SEED, DEFAULT_FEE_BPS, DEFAULT_POOL_SEED, LP_SEED, TOKEN_DECIMALS, TREASURY_X_SEED,
        TREASURY_Y_SEED, ZERO_AMOUNT,
    },
    anchor_lang::AccountDeserialize,
    anchor_spl::associated_token,
    litesvm::LiteSVM,
    litesvm_token::{
        get_spl_account,
        spl_token::state::{Account as TokenAccount, Mint},
        CreateMint,
    },
    solana_keypair::Keypair,
    solana_message::{Instruction, Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

mod constants;
mod ix_handlers;
use constants::*;
use ix_handlers::*;

fn send(
    svm: &mut LiteSVM,
    ixs: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
) -> litesvm::types::TransactionResult {
    svm.expire_blockhash();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

fn setup() -> (
    LiteSVM,
    Keypair,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
) {
    let program_id = amm_video::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/amm_video.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), TEST_AIRDROP_LAMPORTS).unwrap();

    let mint_x = CreateMint::new(&mut svm, &payer)
        .decimals(TOKEN_DECIMALS)
        .authority(&payer.pubkey())
        .send()
        .unwrap();

    let mint_y = CreateMint::new(&mut svm, &payer)
        .decimals(TOKEN_DECIMALS)
        .authority(&payer.pubkey())
        .send()
        .unwrap();

    let config = Pubkey::find_program_address(
        &[CONFIG_SEED, &DEFAULT_POOL_SEED.to_le_bytes()],
        &amm_video::id(),
    )
    .0;
    let mint_lp = Pubkey::find_program_address(&[LP_SEED, config.as_ref()], &amm_video::id()).0;
    let vault_x = associated_token::get_associated_token_address(&config, &mint_x);
    let vault_y = associated_token::get_associated_token_address(&config, &mint_y);
    let treasury_x =
        Pubkey::find_program_address(&[TREASURY_X_SEED, config.as_ref()], &amm_video::id()).0;
    let treasury_y =
        Pubkey::find_program_address(&[TREASURY_Y_SEED, config.as_ref()], &amm_video::id()).0;

    (
        svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, treasury_x, treasury_y,
    )
}

fn token_amount(svm: &LiteSVM, account: &Pubkey) -> u64 {
    get_spl_account::<TokenAccount>(svm, account)
        .unwrap()
        .amount
}

fn mint_supply(svm: &LiteSVM, mint: &Pubkey) -> u64 {
    get_spl_account::<Mint>(svm, mint).unwrap().supply
}

fn config_state(svm: &LiteSVM, config: &Pubkey) -> amm_video::Config {
    let account = svm.get_account(config).unwrap();
    let mut data = account.data.as_slice();
    amm_video::Config::try_deserialize(&mut data).unwrap()
}

#[test]
fn test_initialize() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, treasury_x, treasury_y) =
        setup();

    let instruction = create_initialise_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        config,
        mint_lp,
        vault_x,
        vault_y,
        treasury_x,
        treasury_y,
        DEFAULT_FEE_BPS,
    );
    let res = send(&mut svm, &[instruction], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");

    let state = config_state(&svm, &config);
    assert_eq!(state.fee, DEFAULT_FEE_BPS);
    assert_eq!(state.treasury_x, treasury_x);
    assert_eq!(state.treasury_y, treasury_y);
    assert_eq!(mint_supply(&svm, &mint_lp), ZERO_AMOUNT);
    assert_eq!(token_amount(&svm, &vault_x), ZERO_AMOUNT);
    assert_eq!(token_amount(&svm, &vault_y), ZERO_AMOUNT);
    assert_eq!(token_amount(&svm, &treasury_x), ZERO_AMOUNT);
    assert_eq!(token_amount(&svm, &treasury_y), ZERO_AMOUNT);
}

#[test]
fn test_deposit() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, treasury_x, treasury_y) =
        setup();
    let init_ix = create_initialise_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        config,
        mint_lp,
        vault_x,
        vault_y,
        treasury_x,
        treasury_y,
        DEFAULT_FEE_BPS,
    );
    let deposit_ix = create_deposit_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        TEST_INITIAL_LP_AMOUNT,
        TEST_INITIAL_LIQUIDITY,
        TEST_INITIAL_LIQUIDITY,
    );

    let res = send(&mut svm, &[init_ix, deposit_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");

    let user_lp = associated_token::get_associated_token_address(&payer.pubkey(), &mint_lp);
    assert_eq!(token_amount(&svm, &vault_x), TEST_INITIAL_LIQUIDITY);
    assert_eq!(token_amount(&svm, &vault_y), TEST_INITIAL_LIQUIDITY);
    assert_eq!(mint_supply(&svm, &mint_lp), TEST_INITIAL_LP_AMOUNT);
    assert_eq!(token_amount(&svm, &user_lp), TEST_INITIAL_LP_AMOUNT);
}

#[test]
fn test_second_deposit_uses_token_precision() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, treasury_x, treasury_y) =
        setup();
    let init_ix = create_initialise_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        config,
        mint_lp,
        vault_x,
        vault_y,
        treasury_x,
        treasury_y,
        DEFAULT_FEE_BPS,
    );
    let first_deposit_ix = create_deposit_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        TEST_INITIAL_LP_AMOUNT,
        TEST_INITIAL_LIQUIDITY,
        TEST_INITIAL_LIQUIDITY,
    );
    let second_deposit_ix = create_deposit_ix_from_existing(
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        TEST_SECOND_LP_AMOUNT,
        TEST_SECOND_LIQUIDITY,
        TEST_SECOND_LIQUIDITY,
    );

    let res = send(
        &mut svm,
        &[init_ix, first_deposit_ix, second_deposit_ix],
        &payer,
        &[&payer],
    );
    assert!(res.is_ok(), "{res:?}");
    assert_eq!(
        token_amount(&svm, &vault_x),
        TEST_INITIAL_LIQUIDITY + TEST_SECOND_LIQUIDITY
    );
    assert_eq!(
        token_amount(&svm, &vault_y),
        TEST_INITIAL_LIQUIDITY + TEST_SECOND_LIQUIDITY
    );
    assert_eq!(
        mint_supply(&svm, &mint_lp),
        TEST_INITIAL_LP_AMOUNT + TEST_SECOND_LP_AMOUNT
    );
}

#[test]
fn test_withdraw() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, treasury_x, treasury_y) =
        setup();
    let init_ix = create_initialise_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        config,
        mint_lp,
        vault_x,
        vault_y,
        treasury_x,
        treasury_y,
        DEFAULT_FEE_BPS,
    );
    let deposit_ix = create_deposit_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        TEST_INITIAL_LP_AMOUNT,
        TEST_INITIAL_LIQUIDITY,
        TEST_INITIAL_LIQUIDITY,
    );
    let withdraw_ix = create_withdraw_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );

    let res = send(&mut svm, &[init_ix, deposit_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");
    assert_eq!(mint_supply(&svm, &mint_lp), TEST_INITIAL_LP_AMOUNT);
    assert_eq!(token_amount(&svm, &vault_x), TEST_INITIAL_LIQUIDITY);
    let res = send(&mut svm, &[withdraw_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");

    let user_x = associated_token::get_associated_token_address(&payer.pubkey(), &mint_x);
    let user_y = associated_token::get_associated_token_address(&payer.pubkey(), &mint_y);
    let user_lp = associated_token::get_associated_token_address(&payer.pubkey(), &mint_lp);
    assert_eq!(token_amount(&svm, &vault_x), TEST_WITHDRAWN_VAULT_BALANCE);
    assert_eq!(token_amount(&svm, &vault_y), TEST_WITHDRAWN_VAULT_BALANCE);
    assert_eq!(token_amount(&svm, &user_x), TEST_WITHDRAWN_USER_BALANCE);
    assert_eq!(token_amount(&svm, &user_y), TEST_WITHDRAWN_USER_BALANCE);
    assert_eq!(token_amount(&svm, &user_lp), TEST_WITHDRAWN_LP_BALANCE);
}

#[test]
fn test_swap() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, treasury_x, treasury_y) =
        setup();
    let init_ix = create_initialise_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        config,
        mint_lp,
        vault_x,
        vault_y,
        treasury_x,
        treasury_y,
        DEFAULT_FEE_BPS,
    );
    let deposit_ix = create_deposit_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        TEST_INITIAL_LP_AMOUNT,
        TEST_INITIAL_LIQUIDITY,
        TEST_INITIAL_LIQUIDITY,
    );
    let swap_ix = create_swap_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        treasury_x,
        treasury_y,
        true,
        TEST_SWAP_AMOUNT_IN,
        TEST_SWAP_MINIMUM_OUT,
    );

    let res = send(&mut svm, &[init_ix, deposit_ix, swap_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");
    assert_eq!(token_amount(&svm, &vault_x), TEST_FIRST_SWAP_VAULT_BALANCE);
    assert!(token_amount(&svm, &vault_y) < TEST_INITIAL_LIQUIDITY);
    assert_eq!(token_amount(&svm, &treasury_x), TEST_SWAP_FEE_AMOUNT);
    assert_eq!(token_amount(&svm, &treasury_y), ZERO_AMOUNT);
}

#[test]
fn test_swap_reverse_direction() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, treasury_x, treasury_y) =
        setup();
    let init_ix = create_initialise_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        config,
        mint_lp,
        vault_x,
        vault_y,
        treasury_x,
        treasury_y,
        DEFAULT_FEE_BPS,
    );
    let deposit_ix = create_deposit_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        TEST_INITIAL_LP_AMOUNT,
        TEST_INITIAL_LIQUIDITY,
        TEST_INITIAL_LIQUIDITY,
    );
    let swap_ix = create_swap_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        treasury_x,
        treasury_y,
        false,
        TEST_SWAP_AMOUNT_IN,
        TEST_SWAP_MINIMUM_OUT,
    );

    let res = send(&mut svm, &[init_ix, deposit_ix, swap_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");
    assert_eq!(token_amount(&svm, &vault_y), TEST_FIRST_SWAP_VAULT_BALANCE);
    assert!(token_amount(&svm, &vault_x) < TEST_INITIAL_LIQUIDITY);
    assert_eq!(token_amount(&svm, &treasury_x), ZERO_AMOUNT);
    assert_eq!(token_amount(&svm, &treasury_y), TEST_SWAP_FEE_AMOUNT);
}

#[test]
fn test_collect_fees() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, treasury_x, treasury_y) =
        setup();
    let init_ix = create_initialise_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        config,
        mint_lp,
        vault_x,
        vault_y,
        treasury_x,
        treasury_y,
        DEFAULT_FEE_BPS,
    );
    let deposit_ix = create_deposit_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        TEST_INITIAL_LP_AMOUNT,
        TEST_INITIAL_LIQUIDITY,
        TEST_INITIAL_LIQUIDITY,
    );
    let swap_ix = create_swap_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        treasury_x,
        treasury_y,
        true,
        TEST_SWAP_AMOUNT_IN,
        TEST_SWAP_MINIMUM_OUT,
    );
    let collect_ix = create_collect_fees_ix(
        &mut svm, &payer, mint_x, mint_y, config, treasury_x, treasury_y,
    );

    let res = send(
        &mut svm,
        &[init_ix, deposit_ix, swap_ix, collect_ix],
        &payer,
        &[&payer],
    );
    assert!(res.is_ok(), "{res:?}");

    let user_x = associated_token::get_associated_token_address(&payer.pubkey(), &mint_x);
    assert_eq!(token_amount(&svm, &treasury_x), ZERO_AMOUNT);
    assert_eq!(token_amount(&svm, &treasury_y), ZERO_AMOUNT);
    assert_eq!(token_amount(&svm, &user_x), TEST_COLLECTED_USER_BALANCE);
}

#[test]
fn test_invalid_fee_is_rejected() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, treasury_x, treasury_y) =
        setup();
    let init_ix = create_initialise_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        config,
        mint_lp,
        vault_x,
        vault_y,
        treasury_x,
        treasury_y,
        TEST_INVALID_FEE_BPS,
    );

    let res = send(&mut svm, &[init_ix], &payer, &[&payer]);
    assert!(res.is_err());
}

#[test]
fn test_zero_deposit_is_rejected() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, treasury_x, treasury_y) =
        setup();
    let init_ix = create_initialise_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        config,
        mint_lp,
        vault_x,
        vault_y,
        treasury_x,
        treasury_y,
        DEFAULT_FEE_BPS,
    );
    let deposit_ix = create_deposit_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        ZERO_AMOUNT,
        TEST_INITIAL_LIQUIDITY,
        TEST_INITIAL_LIQUIDITY,
    );

    let res = send(&mut svm, &[init_ix, deposit_ix], &payer, &[&payer]);
    assert!(res.is_err());
}

#[test]
fn test_swap_slippage_is_rejected() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, treasury_x, treasury_y) =
        setup();
    let init_ix = create_initialise_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        config,
        mint_lp,
        vault_x,
        vault_y,
        treasury_x,
        treasury_y,
        DEFAULT_FEE_BPS,
    );
    let deposit_ix = create_deposit_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        TEST_INITIAL_LP_AMOUNT,
        TEST_INITIAL_LIQUIDITY,
        TEST_INITIAL_LIQUIDITY,
    );
    let swap_ix = create_swap_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        treasury_x,
        treasury_y,
        true,
        TEST_SWAP_AMOUNT_IN,
        TEST_SWAP_HIGH_MINIMUM_OUT,
    );

    let res = send(&mut svm, &[init_ix, deposit_ix, swap_ix], &payer, &[&payer]);
    assert!(res.is_err());
}
