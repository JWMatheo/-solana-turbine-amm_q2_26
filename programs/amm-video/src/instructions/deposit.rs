use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{mint_to, transfer, Mint, MintTo, Token, TokenAccount, Transfer},
};
use constant_product_curve::ConstantProduct;

use crate::{
    constants::{CONFIG_SEED, LP_SEED, TOKEN_PRECISION, ZERO_AMOUNT},
    error::AmmError,
    state::Config,
    token_side::{select_for_side, TokenSide},
};

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
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
        seeds = [LP_SEED, config.key().as_ref()],
        bump = config.lp_bump,
    )]
    pub mint_lp: Account<'info, Mint>,
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
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = mint_lp,
        associated_token::authority = user,
    )]
    pub user_lp: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

impl<'info> Deposit<'info> {
    pub fn deposit(
        &mut self,
        amount: u64, // Amount of LP tokens that the user wants to "claim"
        max_x: u64,  // Maximum amount of token X that the user is willing to deposit
        max_y: u64,  // Maximum amount of token Y that the user is willing to deposit
    ) -> Result<()> {
        require!(!self.config.locked, AmmError::PoolLocked);
        require_neq!(amount, ZERO_AMOUNT, AmmError::InvalidAmount);

        let (x, y) = if self.mint_lp.supply == ZERO_AMOUNT
            && self.vault_x.amount == ZERO_AMOUNT
            && self.vault_y.amount == ZERO_AMOUNT
        {
            require!(
                max_x > ZERO_AMOUNT && max_y > ZERO_AMOUNT,
                AmmError::InvalidAmount
            );
            (max_x, max_y)
        } else {
            let amounts = ConstantProduct::xy_deposit_amounts_from_l(
                self.vault_x.amount,
                self.vault_y.amount,
                self.mint_lp.supply,
                amount,
                TOKEN_PRECISION,
            )
            .map_err(AmmError::from)?;

            require!(
                amounts.x <= max_x && amounts.y <= max_y,
                AmmError::SlippageExceeded
            );

            (amounts.x, amounts.y)
        };

        // deposit token x
        self.deposit_tokens(TokenSide::X, x)?;
        // deposit token y
        self.deposit_tokens(TokenSide::Y, y)?;
        // mint lp tokens
        self.mint_lp_tokens(amount)
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

        let cpi_program = self.token_program.key();

        let cpi_accounts = Transfer {
            from,
            to,
            authority: self.user.to_account_info(),
        };

        let ctx = CpiContext::new(cpi_program, cpi_accounts);

        transfer(ctx, amount)
    }

    fn mint_lp_tokens(&self, amount: u64) -> Result<()> {
        let cpi_program = self.token_program.key();

        let cpi_accounts = MintTo {
            mint: self.mint_lp.to_account_info(),
            to: self.user_lp.to_account_info(),
            authority: self.config.to_account_info(),
        };

        let signer_seeds: &[&[&[u8]]] = &[&[
            CONFIG_SEED,
            &self.config.seed.to_le_bytes(),
            &[self.config.config_bump],
        ]];

        let ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer_seeds);

        mint_to(ctx, amount)
    }
}
