use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{transfer, Mint, Token, TokenAccount, Transfer},
};
use constant_product_curve::{ConstantProduct, LiquidityPair};

use crate::{
    constants::{
        CONFIG_SEED, LP_SEED, MAX_FEE_BPS, TOKEN_DECIMALS, TREASURY_X_SEED, TREASURY_Y_SEED,
        ZERO_AMOUNT, ZERO_FEE_BPS,
    },
    error::AmmError,
    state::Config,
    token_side::{select_for_side, TokenSide},
};

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub mint_x: Box<Account<'info, Mint>>,
    pub mint_y: Box<Account<'info, Mint>>,
    #[account(
        has_one = mint_x,
        has_one = mint_y,
        seeds = [CONFIG_SEED, config.seed.to_le_bytes().as_ref()],
        bump = config.config_bump
    )]
    pub config: Account<'info, Config>,
    #[account(
        seeds = [LP_SEED, config.key().as_ref()],
        bump = config.lp_bump,
    )]
    pub mint_lp: Box<Account<'info, Mint>>,
    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = config,
    )]
    pub vault_x: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = config,
    )]
    pub vault_y: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        address = config.treasury_x,
        seeds = [TREASURY_X_SEED, config.key().as_ref()],
        bump,
        token::mint = mint_x,
        token::authority = config,
    )]
    pub treasury_x: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        address = config.treasury_y,
        seeds = [TREASURY_Y_SEED, config.key().as_ref()],
        bump,
        token::mint = mint_y,
        token::authority = config,
    )]
    pub treasury_y: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = user,
    )]
    pub user_x: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = user,
    )]
    pub user_y: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl<'info> Swap<'info> {
    pub fn swap(&mut self, is_x: bool, amount: u64, min: u64) -> Result<()> {
        require!(!self.config.locked, AmmError::PoolLocked);
        require!(amount > ZERO_AMOUNT, AmmError::InvalidAmount);
        require!(self.config.fee <= MAX_FEE_BPS, AmmError::FeePercentErr);
        let input_side = TokenSide::from(is_x);

        let fee_bps = MAX_FEE_BPS
            .checked_sub(self.config.fee)
            .ok_or_else(|| error!(AmmError::InvalidFee))?;
        let amount_after_fee = ((amount as u128)
            .checked_mul(fee_bps as u128)
            .ok_or_else(|| error!(AmmError::Overflow))?
            .checked_div(MAX_FEE_BPS as u128)
            .ok_or_else(|| error!(AmmError::Overflow))?) as u64;
        let fee_amount = amount
            .checked_sub(amount_after_fee)
            .ok_or_else(|| error!(AmmError::Underflow))?;
        require!(amount_after_fee > ZERO_AMOUNT, AmmError::InvalidAmount);

        let mut curve = ConstantProduct::init(
            self.vault_x.amount,
            self.vault_y.amount,
            self.mint_lp.supply,
            ZERO_FEE_BPS,
            Some(TOKEN_DECIMALS),
        )
        .map_err(AmmError::from)?;

        let p = select_for_side(input_side, LiquidityPair::X, LiquidityPair::Y);

        let swap_result: constant_product_curve::SwapResult = curve
            .swap(p, amount_after_fee, min)
            .map_err(AmmError::from)?;

        self.deposit_tokens(input_side, swap_result.deposit)?;
        self.deposit_fee(input_side, fee_amount)?;
        self.withdraw_tokens(input_side, swap_result.withdraw)
    }

    fn deposit_tokens(&self, side: TokenSide, amount: u64) -> Result<()> {
        let (from, to) = select_for_side(
            side,
            (
                self.user_x.to_account_info(),
                self.vault_x.to_account_info(),
            ),
            (
                self.user_y.to_account_info(),
                self.vault_y.to_account_info(),
            ),
        );

        transfer(
            CpiContext::new(
                self.token_program.key(),
                Transfer {
                    from,
                    to,
                    authority: self.user.to_account_info(),
                },
            ),
            amount,
        )
    }

    fn deposit_fee(&self, side: TokenSide, amount: u64) -> Result<()> {
        if amount == ZERO_AMOUNT {
            return Ok(());
        }

        let (from, to) = select_for_side(
            side,
            (
                self.user_x.to_account_info(),
                self.treasury_x.to_account_info(),
            ),
            (
                self.user_y.to_account_info(),
                self.treasury_y.to_account_info(),
            ),
        );

        transfer(
            CpiContext::new(
                self.token_program.key(),
                Transfer {
                    from,
                    to,
                    authority: self.user.to_account_info(),
                },
            ),
            amount,
        )
    }

    fn withdraw_tokens(&self, input_side: TokenSide, amount: u64) -> Result<()> {
        let output_side = input_side.opposite();
        let (from, to) = select_for_side(
            output_side,
            (
                self.vault_x.to_account_info(),
                self.user_x.to_account_info(),
            ),
            (
                self.vault_y.to_account_info(),
                self.user_y.to_account_info(),
            ),
        );

        transfer(
            CpiContext::new_with_signer(
                self.token_program.key(),
                Transfer {
                    from,
                    to,
                    authority: self.config.to_account_info(),
                },
                &[&[
                    CONFIG_SEED,
                    &self.config.seed.to_le_bytes(),
                    &[self.config.config_bump],
                ]],
            ),
            amount,
        )
    }
}
