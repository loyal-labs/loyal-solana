use anchor_lang::prelude::ProgramError;
use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::solana_program::program::invoke_signed;
use ephemeral_rollups_sdk::anchor::{commit, delegate, ephemeral};
use ephemeral_rollups_sdk::cpi::DelegateConfig;
use ephemeral_rollups_sdk::ephem::{commit_accounts, commit_and_undelegate_accounts};

declare_id!("9Sg7UG96gVEPChRdT5Y6DKeaiMV5eTYm1phsWArna98t");

const ORACLE_IDENTITY: Pubkey = pubkey!("62JLkPeE4oG65LRB3W3m52RVicmYq3xFHdv7TecCsPj5");

pub const STATUS_WAITING_FOR_DELEGATION: u8 = 0;
pub const STATUS_PENDING: u8 = 1;
pub const STATUS_DONE:    u8 = 2;
pub const STATUS_ERROR:   u8 = 3;
pub const MAX_RESPONSE_LEN: usize = 4096;
pub const MAX_TEXT_LEN: usize = 2048;
pub const MAX_ACCOUNT_METAS: usize = 8;
pub const MAX_CALLBACK_METAS: usize = 8; 
pub const INTERACTION_SEED: &[u8] = b"interaction";


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
    #[msg("Illegal signer meta.")]
    IllegalSignerMeta,
    #[msg("Missing remaining account.")]
    MissingRemainingAccount,
    #[msg("Writability mismatch.")]
    WritabilityMismatch,
    #[msg("Oracle must not appear in remaining accounts.")]
    OracleMustNotAppearInRemaining,
    #[msg("Identity must be a signer.")]
    IdentityNotSigner,
    #[msg("Program is not executable.")]
    ProgramNotExecutable,
    #[msg("Unauthorized.")]
    Unauthorized
}

#[error_code]
pub enum ErrorCode {
    #[msg("Provided interaction_id is not the next available id for creation.")]
    InvalidInteractionId,
    #[msg("Interaction id does not match the PDA being updated.")]
    IdMismatch,
    #[msg("Context account does not match the interaction's context.")]
    ContextMismatch,
    #[msg("Supplied text exceeds MAX_TEXT.")]
    TextTooLong,
    #[msg("Too many callback account metas.")]
    TooManyMetas,
    #[msg("Arithmetic overflow.")]
    MathOverflow
}

#[ephemeral]
#[program]
pub mod loyal_oracle {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }

    pub fn create_context(ctx: Context<CreateContext>) -> Result<()> {
        let c = &mut ctx.accounts.context_account;
        c.owner = ctx.accounts.payer.key();
        c.next_interaction = 0;

        Ok(())
    }

    pub fn interact_with_llm(
        ctx: Context<InteractWithLlm>,
        interaction_id: u64,
        text: Option<String>,
        callback_program_id: Pubkey,
        callback_discriminator: [u8; 8],
        account_metas: Option<Vec<StoredAccountMeta>>,
    ) -> Result<()> {
        let i = &mut ctx.accounts.interaction;

        // Detect whether this was freshly created by `init_if_needed`.
        // After a real init, all fields are zeroed except discriminator.
        let is_new = i.created_at == 0;

        if is_new {
            // For creation, require the caller to use the "next" id
            require!(
                interaction_id == ctx.accounts.context_account.next_interaction,
                ErrorCode::InvalidInteractionId
            );

            i.id = interaction_id;
            i.context = ctx.accounts.context_account.key();
            i.user = ctx.accounts.payer.key();
            i.created_at = Clock::get()?.unix_timestamp;
            i.response.clear();

            // On first initialize we also ensure the text starts empty if no input provided.
            if text.is_none() {
                i.status = STATUS_WAITING_FOR_DELEGATION;
                i.text.clear();
            }
        } else {
            // Sanity on update path
            require_eq!(i.id, interaction_id, ErrorCode::IdMismatch);
            require_keys_eq!(i.context, ctx.accounts.context_account.key(), ErrorCode::ContextMismatch);
        }

        // Apply fields that can be set on both create & update:

        // Text + status
        if let Some(t) = text {
            require!(t.len() <= MAX_TEXT_LEN, ErrorCode::TextTooLong);
            i.status = STATUS_PENDING;
            i.text = t;
        } else {
            i.status = STATUS_WAITING_FOR_DELEGATION;
            i.text.clear();
        }

        // Callback wiring
        let metas = account_metas.unwrap_or_default();
        require!(metas.len() <= MAX_ACCOUNT_METAS, ErrorCode::TooManyMetas);
        i.callback_program_id = callback_program_id;
        i.callback_discriminator = callback_discriminator;
        i.callback_account_metas = metas;

        // Only advance the counter if we truly created a new PDA in this call
        if is_new {
            ctx.accounts.context_account.next_interaction =
                ctx.accounts.context_account
                    .next_interaction
                    .checked_add(1)
                    .ok_or(ErrorCode::MathOverflow)?;
        }

        Ok(())
    }

    pub fn callback_from_llm<'info>(
        ctx: Context<'_, '_, '_, 'info, CallbackFromLlm<'info>>,
        response: String,
        is_processed: bool,
    ) -> Result<()> {
        // only our oracle signer may call this
        require_keys_eq!(
            ctx.accounts.oracle_signer.key(),
            ORACLE_IDENTITY,
            CustomError::Unauthorized
        );

        // Verify the intended callback program matches what was recorded on the interaction
        require_keys_eq!(
            ctx.accounts.program.key(),
            ctx.accounts.interaction.callback_program_id,
            CustomError::WrongCallbackProgram
        );
        require!(ctx.accounts.program.executable, CustomError::ProgramNotExecutable);

        // Don’t allow double-processing
        require!(
            ctx.accounts.interaction.status != STATUS_DONE,
            CustomError::AlreadyProcessed
        );

        // persist the response into the interaction PDA
        require!(response.as_bytes().len() <= MAX_RESPONSE_LEN, CustomError::ResponseTooLong);

        let interaction = &mut ctx.accounts.interaction;
        interaction.response = response.clone();
        interaction.status = if is_processed { STATUS_DONE } else { STATUS_PENDING };

        // CPI data: discriminator + borsh(args)
        let args = CallbackArgs { response, is_processed };
        let data = [
            interaction.callback_discriminator.to_vec(), 
            args.try_to_vec()?, 
        ]
        .concat();

        // Build metas (IDENTITY FIRST), followed by the recorded metas, in the recorded order
        //    Identity must be a signer of the inner ix.
        let mut metas: Vec<AccountMeta> = Vec::with_capacity(1 + interaction.callback_account_metas.len());
        metas.push(AccountMeta {
            pubkey: ctx.accounts.identity.key(),
            is_signer: true,
            is_writable: false,
        });
        // Ensure none of the recorded metas claim `is_signer = true` (we can’t sign for them)
        for m in &interaction.callback_account_metas {
            require!(!m.is_signer, CustomError::IllegalSignerMeta);
            metas.push(AccountMeta {
                pubkey: m.pubkey,
                is_signer: false,
                is_writable: m.is_writable,
            });
        }

        // 7) Verify the provided remaining accounts cover those metas (by pubkey) and assemble
        //    AccountInfos in the SAME ORDER: identity, then each meta, then the program account.
        let mut infos: Vec<AccountInfo<'info>> = Vec::with_capacity(1 + ctx.remaining_accounts.len() + 1);
        infos.push(ctx.accounts.identity.to_account_info());

        for m in &interaction.callback_account_metas {
            // find corresponding AccountInfo in remaining_accounts
            let ai = ctx
                .remaining_accounts
                .iter()
                .find(|ai| ai.key() == m.pubkey)
                .ok_or(CustomError::MissingRemainingAccount)?
                .to_account_info();
            // assert writability matches.
            if m.is_writable {
                require!(ai.is_writable, CustomError::WritabilityMismatch);
            }
            infos.push(ai);
        }

        require!(
            !ctx.remaining_accounts.iter().any(|acc| acc.key() == ctx.accounts.oracle_signer.key()),
            CustomError::OracleMustNotAppearInRemaining
        );

        let instruction = Instruction {
            program_id: ctx.accounts.program.key(),
            accounts: metas,
            data,
        };

        // program account must be included in the infos slice
        infos.push(ctx.accounts.program.to_account_info());

        // 8) Sign as the identity PDA
        let identity_bump = ctx.bumps.identity;
        invoke_signed(
            &instruction,
            &infos,
            &[&[b"identity", &[identity_bump]]],
        )?;

        Ok(()) 
    }

    pub fn callback_from_oracle(ctx: Context<CallbackFromOracle>, response: String, is_processed: bool) -> Result<()> {
        // Identity must appear as a signer on the INNER instruction (enforced by our invoke_signed)
        require!(ctx.accounts.identity.to_account_info().is_signer, CustomError::IdentityNotSigner);
        msg!("Callback response: {}", response);
        msg!("Processed? {}", is_processed);
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
                validator: Some(pubkey!("MUS3hc9TCw4cGC12vHNoYcCGzJG1txjgQLZWVoeNHNd")),
            },
        )?;
        Ok(())
    }

    pub fn delegate_interaction(ctx: Context<DelegateInteraction>, interaction_id: u64) -> Result<()> {
        ctx.accounts.delegate_interaction(
            &ctx.accounts.payer,
            &[
                INTERACTION_SEED,
                ctx.accounts.context_account.key().as_ref(),
                &interaction_id.to_le_bytes(),
            ],
            DelegateConfig {
                commit_frequency_ms: 0,
                validator: Some(pubkey!("MUS3hc9TCw4cGC12vHNoYcCGzJG1txjgQLZWVoeNHNd")),
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
pub struct CreateContext<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        // 8 discr + 32 owner + (fields)
        space = 8 + 32 + 8 + 4,
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
#[instruction(interaction_id: u64)]
pub struct InteractWithLlm<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    // MUST be the owner; ensures "context uniquely connected to the user"
    #[account(
        mut,
        constraint = context_account.owner == payer.key() @ CustomError::ContextOwnerMismatch
    )]
    pub context_account: Account<'info, ContextAccount>,

    /// CHECK: fresh PDA per (context, interaction_id)
    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + Interaction::INIT_SPACE,
        seeds = [
            INTERACTION_SEED,
            context_account.key().as_ref(),
            &interaction_id.to_le_bytes(),
        ],
        bump
    )]
    pub interaction: Account<'info, Interaction>,

    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
struct CallbackArgs {
    response: String,
    is_processed: bool,
}

#[derive(Accounts)]
pub struct CallbackFromLlm<'info> {
    /// The oracle's off-chain signer
    #[account(mut)]
    pub oracle_signer: Signer<'info>,

    /// Identity PDA of THIS program: signs the inner CPI
    #[account(seeds = [b"identity"], bump)]
    pub identity: Account<'info, Identity>,

    /// must be owned by this program
    #[account(mut)]
    pub interaction: Account<'info, Interaction>,

    /// The target program to receive the callback CPI.
    /// CHECK:
    pub program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct CallbackFromOracle<'info> {
    /// Identity PDA must be the signer of the inner call
    #[account(seeds = [b"identity"], bump)]
    pub identity: Account<'info, Identity>,
}

#[delegate]
#[derive(Accounts)]
#[instruction(interaction_id: u64)]
pub struct DelegateInteraction<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: the correct interaction account
    #[account(
        mut, del,
        seeds = [INTERACTION_SEED, context_account.key().as_ref(), &interaction_id.to_le_bytes()],
        bump
    )]
    /// CHECK: we only use seeds + delegation, no need to deserialize here
    pub interaction: AccountInfo<'info>,

    /// CHECK: we accept any context
    pub context_account: AccountInfo<'info>,
}

/// Accounts

#[account]
pub struct ContextAccount {
    pub owner: Pubkey,
    pub next_interaction: u64,
}

impl ContextAccount { pub fn seed() -> &'static [u8] { b"context" } }

#[account]
#[derive(InitSpace)]
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
    #[max_len(MAX_TEXT_LEN)]
    pub text: String,
    #[max_len(MAX_RESPONSE_LEN)]
    pub response: String,
    #[max_len(MAX_CALLBACK_METAS)]
    pub callback_account_metas: Vec<StoredAccountMeta>,

}

#[derive(InitSpace, AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct StoredAccountMeta {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl StoredAccountMeta {
    pub const SIZE: usize = 32 + 1 + 1;
}

#[account]
pub struct Counter {
    pub count: u32,
}

#[account]
pub struct Identity {}
