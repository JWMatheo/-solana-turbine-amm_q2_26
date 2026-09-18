use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{transfer, Mint, Token, TokenAccount, Transfer},
};

use crate::{
    constants::{CONFIG_SEED, TREASURY_X_SEED, TREASURY_Y_SEED, ZERO_AMOUNT},
    error::AmmError,
    state::Config,
    token_side::{select_for_side, TokenSide},
};

#[derive(Accounts)]
pub struct CollectFees<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    pub mint_x: Account<'info, Mint>,
    pub mint_y: Account<'info, Mint>,
    #[account(
        has_one = mint_x,
        has_one = mint_y,
        seeds = [CONFIG_SEED, config.seed.to_le_bytes().as_ref()],
        bump = config.config_bump,
    )]
    pub config: Account<'info, Config>,
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
        init_if_needed,
        payer = authority,
        associated_token::mint = mint_x,
        associated_token::authority = authority,
    )]
    pub destination_x: Box<Account<'info, TokenAccount>>,
    #[account(
        init_if_needed,
        payer = authority,
        associated_token::mint = mint_y,
        associated_token::authority = authority,
    )]
    pub destination_y: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl<'info> CollectFees<'info> {
    pub fn collect_fees(&self) -> Result<()> {
        let configured_authority = self
            .config
            .authority
            .ok_or_else(|| error!(AmmError::NoAuthoritySet))?;
        require_keys_eq!(
            configured_authority,
            self.authority.key(),
            AmmError::InvalidAuthority
        );

        self.collect_token(TokenSide::X)?;
        self.collect_token(TokenSide::Y)
    }

    fn collect_token(&self, side: TokenSide) -> Result<()> {
        let (from, to, amount) = select_for_side(
            side,
            (
                self.treasury_x.to_account_info(),
                self.destination_x.to_account_info(),
                self.treasury_x.amount,
            ),
            (
                self.treasury_y.to_account_info(),
                self.destination_y.to_account_info(),
                self.treasury_y.amount,
            ),
        );

        if amount == ZERO_AMOUNT {
            return Ok(());
        }

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
