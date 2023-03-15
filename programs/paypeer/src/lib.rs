use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod paypeer {
    use super::*;

    pub fn swap(
        ctx: Context<Swap>,
        amount_in: u64,
        min_amount_out: u64,
        swap_token: Pubkey,
    ) -> Result<()> {
        let token_in = if ctx.token_a.key() == &swap_token {
            ctx.token_a.clone()
        } else if ctx.token_b.key() == &swap_token {
            ctx.token_b.clone()
        } else {
            return Err(ProgramError::InvalidArgument);
        };

        let token_out = if token_in.key() == ctx.token_a.key() {
            ctx.token_b.clone()
        } else {
            ctx.token_a.clone()
        };

        let pool_token_supply = ctx.pool_token.amount()?;
        let token_in_balance = token_in.amount()?;
        let token_out_balance = token_out.amount()?;

        let amount_in_with_fee = amount_in - ctx.calculate_fee(amount_in)?;

        let token_in_ratio = amount_in_with_fee as f64 / token_in_balance as f64;
        let token_out_amount = (token_out_balance as f64 * token_in_ratio).floor() as u64;

        if token_out_amount < min_amount_out {
            panic!("Invalid argument");
        }

        let new_token_in_balance = token_in_balance + amount_in_with_fee;
        let new_token_out_balance = token_out_balance - token_out_amount;

        token_in.set_amount(new_token_in_balance)?;
        token_out.set_amount(new_token_out_balance)?;
        ctx.pool_token
            .set_amount(pool_token_supply + token_out_amount)?;

        let fees = ctx.calculate_fee(amount_in)?;
        if fees > 0 {
            let fees_amount = fees / 2;
            ctx.fees_account
                .set_amount(ctx.fees_account.amount()? + fees_amount)?;
            token_in.transfer(&ctx.fees_account, fees_amount)?;
            token_out.transfer(&ctx.fees_account, fees_amount)?;
        }

        Ok(())
    }

    pub fn add_liquidity(
        ctx: Context<AddLiquidity>,
        user_token_a_amount: u64,
        user_token_b_amount: u64,
        min_pool_token_amount: u64,
    ) -> Result<()> {
        let token_a_balance = ctx.token_a.amount()?;
        let token_b_balance = ctx.token_b.amount()?;
        let pool_token_supply = ctx.pool_token.amount()?;

        let (token_a_amount, token_b_amount, pool_token_amount) = if pool_token_supply == 0 {
            (
                user_token_a_amount,
                user_token_b_amount,
                (user_token_a_amount * user_token_b_amount).sqrt(),
            )
        } else {
            let token_a_amount = (user_token_a_amount as f64 * pool_token_supply as f64
                / token_a_balance as f64)
                .floor() as u64;
            let token_b_amount = (user_token_b_amount as f64 * pool_token_supply as f64
                / token_b_balance as f64)
                .floor() as u64;
            let pool_token_amount =
                (user_token_a_amount as f64 * user_token_b_amount as f64 * pool_token_supply as f64
                    / (token_a_balance as f64 * token_b_balance as f64))
                    .floor() as u64;

            (token_a_amount, token_b_amount, pool_token_amount)
        };

        let pool_token_amount = pool_token_amount.min(self.pool_token.amount()?);

        if pool_token_amount < min_pool_token_amount {
            return Err(ProgramError::InvalidArgument);
        }

        ctx.token_a
            .transfer(&mut ctx.accounts.token_a_account, token_a_amount)?;
        ctx.token_b
            .transfer(&mut ctx.accounts.token_b_account, token_b_amount)?;
        ctx.pool_token
            .transfer(&mut ctx.accounts.pool_token_account, pool_token_amount)?;

        let user_pool_token_balance = ctx.accounts.pool_token_account.amount()?;
        let new_token_a_balance = token_a_balance + token_a_amount;
        let new_token_b_balance = token_b_balance + token_b_amount;
        let pool_token_amount_to_return = if pool_token_supply == 0 {
            pool_token_amount
        } else {
            pool_token_amount * user_pool_token_balance / pool_token_supply
        };

        ctx.pool_token.mint_to(
            &mut ctx.accounts.user_token_a_account,
            pool_token_amount_to_return,
        )?;
        ctx.pool_token.mint_to(
            &mut ctx.accounts.user_token_b_account,
            pool_token_amount_to_return,
        )?;

        Ok(())
    }

    pub fn remove_liquidity(
        ctx: Context<RemoveLiquidity>,
        pool_token_amount: u64,
        min_token_a_amount: u64,
        min_token_b_amount: u64,
    ) -> Result<()> {
        let token_a_balance = ctx.token_a.amount()?;
        let token_b_balance = ctx.token_b.amount()?;
        let pool_token_supply = ctx.pool_token.amount()?;

        let user_pool_token_balance = ctx.accounts.pool_token_account.amount()?;
        if user_pool_token_balance < pool_token_amount {
            panic!("invalid argument");
        }
        
        let token_a_amount = token_a_balance * pool_token_amount / pool_token_supply;
        let token_b_amount = token_b_balance * pool_token_amount / pool_token_supply;
        
        if token_a_amount < min_token_a_amount || token_b_amount < min_token_b_amount {
            panic!("invalid argument");
        }

        ctx.accounts
            .token_a_account
            .mint
            .transfer(&mut ctx.accounts.token_a_account, token_a_amount)?;
        ctx.accounts
            .token_b_account
            .mint
            .transfer(&mut ctx.accounts.token_b_account, token_b_amount)?;
        ctx.pool_token
            .burn_from(&mut ctx.accounts.pool_token_account, pool_token_amount)?;

        let new_token_a_balance = token_a_balance - token_a_amount;
        let new_token_b_balance = token_b_balance - token_b_amount;

        ctx.token_a
            .transfer(&mut ctx.accounts.user_token_a_account, token_a_amount)?;
        ctx.token_b
            .transfer(&mut ctx.accounts.user_token_b_account, token_b_amount)?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub token_in: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub token_out: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub pool_token: Box<Account<'info, TokenAccount>>,
    #[account(signer)]
    pub owner: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(mut)]
    pub token_a_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub token_b_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub pool_token_account: Box<Account<'info, TokenAccount>>,
    #[account(signer)]
    pub owner: AccountInfo<'info>,
    #[account(mut)]
    pub user_token_a_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub user_token_b_account: Box<Account<'info, TokenAccount>>,
}

#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    #[account(mut)]
    pub token_a_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub token_b_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub pool_token_account: Box<Account<'info, TokenAccount>>,
    #[account(signer)]
    pub owner: AccountInfo<'info>,
    #[account(mut)]
    pub user_token_a_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub user_token_b_account: Box<Account<'info, TokenAccount>>,
}
