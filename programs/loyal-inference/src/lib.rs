use anchor_lang::prelude::*;
use ephemeral_rollups_sdk::anchor::{commit, delegate, ephemeral};
use ephemeral_rollups_sdk::cpi::DelegateConfig;
use ephemeral_rollups_sdk::ephem::{commit_accounts, commit_and_undelegate_accounts};

declare_id!("3ezv3YP5V83UP6KNqgHgt7NGE6JonkSK32nnbMyFEX4U");

pub const TEST_PDA_SEED: &[u8] = b"loyal-pda-test";

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
    #[account(init_if_needed, payer = user, space = 8 + 4 + 256 + 4 + 256 + 1 + 1, seeds = [TEST_PDA_SEED], bump)]
    pub chat: Account<'info, LoyalChat>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MessageIn<'info> {
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
