use anchor_lang::prelude::*;
use ephemeral_rollups_sdk::anchor::{commit, delegate, ephemeral};
use ephemeral_rollups_sdk::cpi::DelegateConfig;
use ephemeral_rollups_sdk::ephem::{commit_accounts, commit_and_undelegate_accounts};

declare_id!("3ezv3YP5V83UP6KNqgHgt7NGE6JonkSK32nnbMyFEX4U");

pub const TEST_PDA_SEED: &[u8] = b"loyal-pda-test";


#[ephemeral]
#[program]
pub mod loyal_inference {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        let chat = &mut ctx.accounts.chat;
        chat.msg_in = None;
        chat.msg_out = None;
        chat.state = ChatState::Waiting;
        chat.turn = ChatTurn::User;

        msg!("Chat initialized");

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init_if_needed, payer = user, space = 8 + std::mem::size_of::<LoyalChat>())]
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

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq)]
pub enum ChatState {
    Waiting,
    Processing
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq)]
pub enum ChatTurn {
    User,
    Model
}

#[account]
pub struct LoyalChat {
    pub msg_in: Option<Vec<u8>>,
    pub msg_out: Option<Vec<u8>>,
    pub state: ChatState,
    pub turn: ChatTurn
}
