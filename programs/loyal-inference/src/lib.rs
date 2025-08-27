use anchor_lang::prelude::*;
use ephemeral_rollups_sdk::anchor::{commit, delegate, ephemeral};
use ephemeral_rollups_sdk::cpi::DelegateConfig;
use ephemeral_rollups_sdk::ephem::{commit_and_undelegate_accounts};

declare_id!("3ezv3YP5V83UP6KNqgHgt7NGE6JonkSK32nnbMyFEX4U");

pub const TEST_PDA_SEED: &[u8] = b"randomized-seed";

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

    /// Delegate the chat account to the delegation program
    pub fn delegate(ctx: Context<DelegateChat>, params: DelegateParams) -> Result<()> {
        let config = DelegateConfig {
            commit_frequency_ms: params.commit_frequency_ms,
            validator: params.validator,
        };
        ctx.accounts.delegate_chat(
            &ctx.accounts.user,
            &[TEST_PDA_SEED, ctx.accounts.user.key().to_bytes().as_slice()],
            config, 
        )?;
        Ok(())
    }
 

    // Undelegate the chat account
    pub fn undelegate(ctx: Context<Undelegate>) -> Result<()> {
        commit_and_undelegate_accounts(
            &ctx.accounts.payer,
            vec![&ctx.accounts.user.to_account_info()],
            &ctx.accounts.magic_context,
            &ctx.accounts.magic_program,
        )?;
        Ok(())
    }

    pub fn query_delegated(ctx: Context<QueryDelegated>, query: Vec<u8>) -> Result<()> {
        let chat = &mut ctx.accounts.chat;
        chat.msg_in = query;
        //chat.processing = true;
        //chat.user_turn = false;
        msg!("Query: {:?}", chat.msg_in);
        Ok(())
    }

    // Send the query to oracle
    pub fn message_in(ctx: Context<MessageIn>, content: Vec<u8>) -> Result<()> {
        let chat = &mut ctx.accounts.chat;
        chat.msg_in = content;
        chat.processing = true;
        chat.user_turn = false;
        msg!("Message in: {:?}", chat.msg_in);
        Ok(())
    }

    pub fn message_out(ctx: Context<MessageIn>, content: Vec<u8>) -> Result<()> {
        let chat = &mut ctx.accounts.chat;
        chat.msg_out = content;
        chat.processing = false;
        chat.user_turn = true;
        msg!("Message out: {:?}", chat.msg_out);
        Ok(())
    }
}

#[account]
pub struct LoyalChat {
    pub msg_in: Vec<u8>,
    pub msg_out: Vec<u8>,
    pub processing: bool,
    pub user_turn: bool,
}

//TODO:
//-Use array pre-allocation for msg_in and msg_out to reduce the space used
//-Alternatively, use one array for streaming and change the status?
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(init_if_needed, payer = payer, space = 8 + 4 + 256 + 4 + 256 + 1 + 1, seeds = [TEST_PDA_SEED, payer.key().to_bytes().as_slice()], bump)]
    pub chat: Account<'info, LoyalChat>,

    pub system_program: Program<'info, System>,
}

#[delegate]
#[derive(Accounts)]
pub struct DelegateChat<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    /// CHECK The pda to delegate
    #[account(mut, del, seeds = [TEST_PDA_SEED, user.key().to_bytes().as_slice()], bump)]
    pub chat: Account<'info, LoyalChat>,
}


#[commit]
#[derive(Accounts)]
pub struct Undelegate<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut, seeds = [TEST_PDA_SEED, payer.key().to_bytes().as_slice()], bump)]
    pub user: Account<'info, LoyalChat>,
}


#[derive(Accounts)]
pub struct MessageIn<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut, seeds = [TEST_PDA_SEED, payer.key().to_bytes().as_slice()], bump)]
    pub chat: Account<'info, LoyalChat>, 
}

#[derive(Accounts)]
pub struct QueryDelegated<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(seeds = [TEST_PDA_SEED, payer.key().to_bytes().as_slice()], bump)]
    pub chat: Account<'info, LoyalChat>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DelegateParams {
    pub commit_frequency_ms: u32,
    pub validator: Option<Pubkey>,
}

