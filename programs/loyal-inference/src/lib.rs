use anchor_lang::prelude::*;
use ephemeral_rollups_sdk::anchor::{commit, delegate, ephemeral};
use ephemeral_rollups_sdk::cpi::DelegateConfig;
use ephemeral_rollups_sdk::ephem::{commit_accounts, commit_and_undelegate_accounts};

declare_id!("3ezv3YP5V83UP6KNqgHgt7NGE6JonkSK32nnbMyFEX4U");

pub const TEST_PDA_SEED: &[u8] = b"loyal-pda-test-dev";

//TODO:
//- Add a way to set msg_in, msg_out, state, turn

#[ephemeral]
#[program]
pub mod loyal_inference {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let chat = &mut ctx.accounts.chat;
        chat.msg_in = vec![];
        chat.msg_out = vec![];
        chat.processing = false;
        chat.user_turn = true;

        msg!("Chat initialized");

        Ok(())
    }

    /// Delegate the account to the delegation program
    pub fn delegate(ctx: Context<DelegateChat>) -> Result<()> {
        ctx.accounts.delegate_chat(
            &ctx.accounts.payer,
            &[TEST_PDA_SEED],
            DelegateConfig::default(), 
        )?;
        Ok(())
    }

    // Get account from ER
    // pub fn undelegate(ctx: Context<UndelegateChat>) -> Result<()> {
    //     commit_and_undelegate_accounts(
    //         &ctx.accounts.payer,
    //         vec![&ctx.accounts.chat.to_account_info()],
    //         &ctx.accounts.magic_context,
    //         &ctx.accounts.magic_program,
    //     )?;
    //     Ok(())
    // }

    // Send the query to oracle
    pub fn message_in(ctx: Context<MessageIn>, content: Vec<u8>) -> Result<()> {
        let chat = &mut ctx.accounts.chat;
        chat.msg_in = content;
        chat.processing = true;
        chat.user_turn = false;
        msg!("Message in: {:?}", chat.msg_in);
        Ok(())
    }
}


//TODO:
//-Use pre-allocation for msg_in and msg_out to reduce the space used
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(init, payer = payer, space = 8 + 4 + 256 + 4 + 256 + 1 + 1, seeds = [TEST_PDA_SEED], bump)]
    pub chat: Account<'info, LoyalChat>,

    pub system_program: Program<'info, System>,
}

#[delegate]
#[derive(Accounts)]
pub struct DelegateChat<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK The pda to delegate
    #[account(mut, del, seeds = [TEST_PDA_SEED], bump)]
    pub chat: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct MessageIn<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut, seeds = [TEST_PDA_SEED], bump)]
    pub chat: Account<'info, LoyalChat>, 
}

#[account]
pub struct LoyalChat {
    pub msg_in: Vec<u8>,
    pub msg_out: Vec<u8>,
    pub processing: bool,
    pub user_turn: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DelegateParams {
    pub commit_frequency_ms: u32,
    pub validator: Option<Pubkey>,
}