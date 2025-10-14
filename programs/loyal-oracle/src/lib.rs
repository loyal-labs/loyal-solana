use anchor_lang::prelude::ProgramError;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::solana_program::program::invoke_signed;
use ephemeral_rollups_sdk::anchor::{commit, delegate, ephemeral};
use ephemeral_rollups_sdk::cpi::DelegateConfig;
use ephemeral_rollups_sdk::ephem::{commit_accounts, commit_and_undelegate_accounts};

declare_id!("9Sg7UG96gVEPChRdT5Y6DKeaiMV5eTYm1phsWArna98t");

const ORACLE_IDENTITY: Pubkey = pubkey!("62JLkPeE4oG65LRB3W3m52RVicmYq3xFHdv7TecCsPj5");

pub const STATUS_PENDING: u8 = 0;
pub const STATUS_DONE:    u8 = 1;
pub const STATUS_ERROR:   u8 = 2;
pub const MAX_RESPONSE_LEN: usize = 4096;


#[error_code]
pub enum CustomError {
    #[msg("Interaction already processed")]
    AlreadyProcessed,
    #[msg("Wrong callback program")]
    WrongCallbackProgram,
    #[msg("Response too long")]
    ResponseTooLong,
    #[msg("Context owner mismatch")]
    ContextOwnerMismatch,
}

#[ephemeral]
#[program]
pub mod loyal_oracle {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }

    pub fn create_context(ctx: Context<CreateContext>, text: String) -> Result<()> {
        let c = &mut ctx.accounts.context_account;
        c.owner = ctx.accounts.payer.key();
        c.next_interaction = 0;
        c.text = text;

        Ok(())
    }

    pub fn interact_with_llm(
        ctx: Context<InteractWithLlm>,
        text: String,
        callback_program_id: Pubkey,
        callback_discriminator: [u8; 8],
        account_metas: Option<Vec<AccountMeta>>,
    ) -> Result<()> {
        let interaction_ai = ctx.accounts.interaction.to_account_info();
        let payer_ai = ctx.accounts.payer.to_account_info();
        let system_ai = ctx.accounts.system_program.to_account_info();

        // allocate account sized for query + max response (no future realloc)
        let space = Interaction::space(&text, account_metas.as_ref().map_or(0, |v| v.len()));
        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(space);

        // fresh create each time: one PDA per query
        let create_ix = anchor_lang::solana_program::system_instruction::create_account(
            &ctx.accounts.payer.key(),
            &ctx.accounts.interaction.key(),
            lamports,
            space as u64,
            &crate::ID,
        );

        let context_account_seed = ctx.accounts.context_account.key();

        let seeds: &[&[&[u8]]] = &[&[
            Interaction::seed(),
            context_account_seed.as_ref(),
            &ctx.accounts.context_account.next_interaction.to_le_bytes(),
            &[ctx.bumps.interaction],
        ]];

        anchor_lang::solana_program::program::invoke_signed(
            &create_ix,
            &[payer_ai.clone(), interaction_ai.clone(), system_ai.clone()],
            seeds,
        )?;

        // write the new interaction
        {
            let mut data = interaction_ai.try_borrow_mut_data()?;
            let mut i = Interaction::try_deserialize_unchecked(&mut data.as_ref()).unwrap_or_default();
            i.context = ctx.accounts.context_account.key();
            i.user = ctx.accounts.payer.key();
            i.created_at = Clock::get()?.unix_timestamp;
            i.id = ctx.accounts.context_account.next_interaction;
            i.text = text;
            i.callback_program_id = callback_program_id;
            i.callback_discriminator = callback_discriminator;
            i.callback_account_metas = account_metas.unwrap_or_default();
            i.status = STATUS_PENDING;
            i.response = String::new();
            i.try_serialize(&mut data.as_mut())?;
        }

        // next query will create a NEW interaction PDA
        ctx.accounts.context_account.next_interaction = 
            ctx.accounts.context_account.next_interaction.checked_add(1).unwrap();

        Ok(())
    }

    pub fn callback_from_llm<'info>(
        ctx: Context<'_, '_, '_, 'info, CallbackFromLlm<'info>>,
        response: String,
        is_processed: bool,
    ) -> Result<()> {
        // Verify callback program id
        require_keys_eq!(
            ctx.accounts.program.key(),
            ctx.accounts.interaction.callback_program_id,
            CustomError::WrongCallbackProgram
        );
        require!(!ctx.accounts.interaction.status == STATUS_DONE, CustomError::AlreadyProcessed);

        // persist the response into the interaction PDA
        require!(response.as_bytes().len() <= MAX_RESPONSE_LEN, CustomError::ResponseTooLong);
        let interaction = &mut ctx.accounts.interaction;
        interaction.response = response.clone();
        interaction.status = if is_processed { STATUS_DONE } else { STATUS_PENDING };

        let response_data = [
            ctx.accounts.interaction.callback_discriminator.to_vec(),
            response.try_to_vec()?,
        ]
        .concat();

        // Prepare accounts metas
        let mut accounts_metas: Vec<anchor_lang::solana_program::instruction::AccountMeta> =
            vec![anchor_lang::solana_program::instruction::AccountMeta {
                pubkey: ctx.accounts.identity.key(),
                is_signer: true,
                is_writable: false,
            }];
        accounts_metas.extend(
            ctx.accounts
                .interaction
                .callback_account_metas
                .iter()
                .map(
                    |meta| anchor_lang::solana_program::instruction::AccountMeta {
                        pubkey: meta.pubkey,
                        is_signer: meta.is_signer,
                        is_writable: meta.is_writable,
                    },
                ),
        );

        // Verify payer is not in remaining accounts
        if ctx
            .remaining_accounts
            .iter()
            .any(|acc| acc.key().eq(&ctx.accounts.payer.key()))
        {
            return Err(ProgramError::InvalidAccountData.into());
        }


        // CPI to the callback program
        let instruction = Instruction {
            program_id: ctx.accounts.program.key(),
            accounts: accounts_metas,
            data: response_data.to_vec(),
        };
        let mut remaining_accounts: Vec<AccountInfo<'info>> = ctx.remaining_accounts.to_vec();
        remaining_accounts.push(ctx.accounts.identity.to_account_info());
        remaining_accounts.push(ctx.accounts.program.to_account_info());

        let identity_bump = ctx.bumps.identity;
        invoke_signed(
            &instruction,
            &remaining_accounts,
            &[&[b"identity", &[identity_bump]]],
        )?;
        Ok(())
    }

    pub fn callback_from_oracle(ctx: Context<CallbackFromOracle>, response: String, is_processed: bool) -> Result<()> {
        if !ctx.accounts.identity.to_account_info().is_signer {
            return Err(ProgramError::InvalidAccountData.into());
        }
        msg!("Callback response: {:?}", response);
        Ok(())
    }

    pub fn delegate_context(ctx: Context<DelegateContext>) -> Result<()> {
        ctx.accounts.delegate_context_account(
            &ctx.accounts.payer,
            &[
                ContextAccount::seed(),
                &ctx.accounts.payer.key().to_bytes(),
            ],
            DelegateConfig {
                commit_frequency_ms: 0,
                validator: Some(pubkey!("mAGicPQYBMvcYveUZA5F5UNNwyHvfYh5xkLS2Fr1mev")),
            },
        )?;
        Ok(())
    }

    pub fn delegate_interaction(ctx: Context<DelegateInteraction>) -> Result<()> {
        ctx.accounts.delegate_interaction(
            &ctx.accounts.payer,
            &[
                Interaction::seed(),
                &ctx.accounts.payer.key().to_bytes(),
                &ctx.accounts.context_account.key().to_bytes(),
            ],
            DelegateConfig {
                commit_frequency_ms: 0,
                validator: Some(pubkey!("mAGicPQYBMvcYveUZA5F5UNNwyHvfYh5xkLS2Fr1mev")),
            },
        )?;
        Ok(())
    }
}

/// Contexts

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        init_if_needed,
        payer = payer,
        space = 8,
        seeds = [b"identity"],
        bump
    )]
    pub identity: Account<'info, Identity>,
    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + 32,
        seeds = [b"counter"],
        bump
    )]
    pub counter: Account<'info, Counter>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(text: String)]
pub struct CreateContext<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        // 8 discr + 32 owner + (fields)
        space = 8 + 32 + 8 + 4 + text.as_bytes().len(),
        seeds = [ContextAccount::seed(), payer.key().as_ref()],
        bump
    )]
    pub context_account: Account<'info, ContextAccount>,
    pub system_program: Program<'info, System>,
}

#[delegate]
#[derive(Accounts)]
pub struct DelegateContext<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: the concrete context PDA
    #[account(
        mut, del,
        seeds = [ContextAccount::seed(), payer.key().as_ref()],
        bump
    )]
    pub context_account: AccountInfo<'info>,
}

#[derive(Accounts)]
#[instruction(
    text: String, 
    callback_program_id: Pubkey, 
    callback_discriminator: [u8; 8], 
    account_metas: Option<Vec<AccountMeta>>)]
pub struct InteractWithLlm<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    // MUST be the owner; ensures "context uniquely connected to the user"
    #[account(
        mut,
        constraint = context_account.owner == payer.key() @ CustomError::ContextOwnerMismatch
    )]
    pub context_account: Account<'info, ContextAccount>,

    /// CHECK: fresh PDA per (context, next_interaction)
    #[account(
        mut,
        seeds = [
            Interaction::seed(),
            context_account.key().as_ref(),
            &context_account.next_interaction.to_le_bytes()
        ],
        bump
    )]
    pub interaction: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CallbackFromLlm<'info> {
    #[account(mut, address = ORACLE_IDENTITY)]
    pub payer: Signer<'info>,
    #[account(seeds = [b"identity"], bump)]
    pub identity: Account<'info, Identity>,
    /// CHECK: we accept any context
    #[account(mut)]
    pub interaction: Account<'info, Interaction>,
    /// CHECK: the callback program
    pub program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct CallbackFromOracle<'info> {
    #[account(seeds = [b"identity"], bump)]
    pub identity: Account<'info, Identity>,
}

#[delegate]
#[derive(Accounts)]
pub struct DelegateInteraction<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    /// CHECK: the correct interaction account
    #[account(
        mut, del,
        seeds = [Interaction::seed(), payer.key().as_ref(), context_account.key().as_ref()],
        bump
    )]
    pub interaction: AccountInfo<'info>,
    /// CHECK: we accept any context
    pub context_account: AccountInfo<'info>,
}

/// Accounts

#[account]
pub struct ContextAccount {
    pub owner: Pubkey,
    pub next_interaction: u64,
    pub text: String,
}

impl ContextAccount { pub fn seed() -> &'static [u8] { b"context" } }

#[account]
#[derive(Default, Debug)]
pub struct Interaction {
    /// ---- fixed-size header (stable offsets) ----
    pub context: Pubkey,
    pub user: Pubkey,
    pub id: u64,
    pub created_at: i64, // unix timestamp
    pub status: u8,
    pub callback_program_id: Pubkey,
    pub callback_discriminator: [u8; 8],

    /// ---- dynamic fields (variable offsets) ----
    pub text: String,
    pub response: String,
    pub callback_account_metas: Vec<AccountMeta>,

}

impl Interaction {
    pub fn seed() -> &'static [u8] { b"interaction" }

    pub fn space(text: &str, metas_len: usize) -> usize {
        // 8 discr
        // 32 context + 32 user + 8 id + 8 created_at + 1 status
        // 32 callback_program_id + 8 callback_discriminator
        // 4 text len + 4 vec len + (4 + MAX_RESPONSE_LEN) for String
        const BASE: usize = 8 + 32 + 32 + 8 + 8 + 1 + 32 + 8 + 4 + 4 + (4 + MAX_RESPONSE_LEN);
        BASE + text.len() + metas_len * AccountMeta::SIZE
    }
}

#[derive(InitSpace, AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct AccountMeta {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl AccountMeta {
    pub const SIZE: usize = 32 + 1 + 1;
}

#[account]
pub struct Counter {
    pub count: u32,
}

#[account]
pub struct Identity {}
