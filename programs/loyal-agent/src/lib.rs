use anchor_lang::prelude::*;
use anchor_lang::Discriminator;
use loyal_oracle::{ContextAccount, Counter, Identity};

declare_id!("7JD7sixsC9bAbCttL1MxbrfNauXjVsAZciGzuicivons");

#[program]
pub mod loyal_agent {
    use super::*;

    const AGENT_DESC: &str = "You are a helpful assistant.";

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        // for future use
        ctx.accounts.agent.context = ctx.accounts.llm_context.key();

        // context for agent
        let cpi_program = ctx.accounts.oracle_program.to_account_info();
        let cpi_accounts = loyal_oracle::cpi::accounts::CreateChat {
            payer: ctx.accounts.payer.to_account_info(),
            context_account: ctx.accounts.llm_context.to_account_info(),
            counter: ctx.accounts.counter.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        loyal_oracle::cpi::create_chat(cpi_ctx, AGENT_DESC.to_string())?;

        Ok(())
    }

    /// forwards query to oracle
    pub fn query(ctx: Context<Query>, text: String) -> Result<()> {
        let cpi_program = ctx.accounts.oracle_program.to_account_info();
        let cpi_accounts = loyal_oracle::cpi::accounts::Query {
            payer: ctx.accounts.payer.to_account_info(),
            interaction: ctx.accounts.interaction.to_account_info(),
            context_account: ctx.accounts.context_account.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        
        // tells oracle which function to call back
        let disc: [u8; 8] = instruction::Callback::DISCRIMINATOR
            .try_into()
            .expect("Discriminator must be 8 bytes");
        
        loyal_oracle::cpi::query(cpi_ctx, text, ID, disc, None)?;

        Ok(())
    }

    /// callback function for oracle
    pub fn callback(ctx: Context<Callback>, response: String) -> Result<()> {
        // check PDA signature
        if !ctx.accounts.identity.to_account_info().is_signer {
            return Err(ProgramError::InvalidAccountData.into());
        }

        // TODO: Process response here
        msg!("Response: {:?}", response);

        Ok(())
    }
}


/// *******************
/// accounts for instructions
/// *******************

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    
    /// holds state
    #[account(
        init,
        payer = payer,
        space = 8 + 32,
        seeds = [b"agent"],
        bump
    )]
    pub agent: Account<'info, Agent>,

    /// CHECK: Checked in oracle program
    #[account(mut)]
    pub llm_context: AccountInfo<'info>,

    #[account(mut)]
    pub counter: Account<'info, Counter>,

    pub system_program: Program<'info, System>,

    /// CHECK: ensure we call trusted oracle
    #[account(address = loyal_oracle::ID)]
    pub oracle_program: AccountInfo<'info>,
}

#[derive(Accounts)]
#[instruction(text: String)]
pub struct Query<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: Checked in oracle program
    #[account(mut)]
    pub interaction: AccountInfo<'info>,

    /// state acc for correct context
    #[account(seeds = [b"agent"], bump)]
    pub agent: Account<'info, Agent>,
    
    #[account(address = agent.context)]
    pub context_account: Account<'info, ContextAccount>,

    /// CHECK: Checked oracle id
    #[account(address = loyal_oracle::ID)]
    pub oracle_program: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Callback<'info> {
    /// CHECK: Checked in oracle program
    pub identity: Account<'info, Identity>,
}

/// *******************
/// data structures  
/// *******************

#[account]
pub struct Agent {
    /// holds context
    pub context: Pubkey,
}