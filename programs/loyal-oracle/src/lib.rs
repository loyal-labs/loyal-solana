use anchor_lang::prelude::*;
use ephemeral_rollups_sdk::anchor::{delegate, ephemeral, commit};
use ephemeral_rollups_sdk::cpi::DelegateConfig;
use hkdf::Hkdf;
use sha2::Sha256;

declare_id!("9Sg7UG96gVEPChRdT5Y6DKeaiMV5eTYm1phsWArna98t");

const ORACLE_IDENTITY: Pubkey = pubkey!("62JLkPeE4oG65LRB3W3m52RVicmYq3xFHdv7TecCsPj5");

pub const STATUS_WAITING_FOR_DELEGATION: u8 = 0;
pub const STATUS_PENDING: u8 = 1;
pub const STATUS_DONE:    u8 = 2;
pub const STATUS_ERROR:   u8 = 3;
pub const CHAT_SEED: &[u8] = b"chat";
pub const DEPOSIT_PDA_SEED: &[u8] = b"deposit";



#[error_code]
pub enum CustomError {
    #[msg("Context owner mismatch")]
    ContextOwnerMismatch,
    #[msg("Unauthorized.")]
    Unauthorized,
    #[msg("HKDF expand failed.")]
    HkdfExpandFailed,
    #[msg("Provided chat_id is not the next available id for creation.")]
    InvalidChatId,
    #[msg("Chat id does not match the PDA being updated.")]
    ChatIdMismatch,
    #[msg("Context does not match the PDA being updated.")]
    ContextMismatch,
    #[msg("Arithmetic overflow.")]
    MathOverflow
}

#[event]
pub struct DekResponse {
    pub chat: Pubkey,
    pub chat_id: u64,
    pub dek: [u8; 32],
}

#[event]
pub struct StatusChanged {
    pub chat: Pubkey,
    pub chat_id: u64,
    pub status: u8,
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
        c.next_chat_id = 0;

        Ok(())
    }

    pub fn create_chat(
        ctx: Context<CreateChat>,
        chat_id: u64,
        cmk: Pubkey,
        tx_id: Pubkey,
    ) -> Result<()> {
        let c = &mut ctx.accounts.chat;
    
        let is_new = c.created_at == 0;
    
        if is_new {
            // enforce monotonic sequence for this context (same rule you used before)
            require!(
                chat_id == ctx.accounts.context_account.next_chat_id,
                CustomError::InvalidChatId
            );
    
            c.context = ctx.accounts.context_account.key();
            c.user = ctx.accounts.payer.key();
            c.id = chat_id;
            c.created_at = Clock::get()?.unix_timestamp;
            c.status = STATUS_PENDING;
    
            // encryption fields
            c.cmk = cmk;
            c.tx_id = tx_id;
    
            // advance counter once per new PDA
            ctx.accounts.context_account.next_chat_id =
                ctx.accounts.context_account
                    .next_chat_id
                    .checked_add(1)
                    .ok_or(CustomError::MathOverflow)?;
        } else {
            // if chat already exists, we don't do anything
            require_eq!(c.id, chat_id, CustomError::ChatIdMismatch);
            require_keys_eq!(c.context, ctx.accounts.context_account.key(), CustomError::ContextMismatch);
            return Ok(());
        }
    
        Ok(())
    }

    pub fn get_dek(ctx: Context<GetDek>) -> Result<()> {
        let caller_key = ctx.accounts.caller.key();
        let c = &ctx.accounts.chat;
    
        // only chat creator OR the oracle identity
        let is_user = caller_key == c.user;
        let is_oracle = caller_key == ORACLE_IDENTITY;
        require!(is_user || is_oracle, CustomError::Unauthorized);
    
        // HKDF(CMK, info="file:"+tx_id) -> 32 bytes
        let cmk_bytes = c.cmk.to_bytes();    // IKM
        let mut info: [u8; 37] = [0u8; 37];  // "file:" (5) + 32-byte tx_id
        info[..5].copy_from_slice(b"file:");
        info[5..].copy_from_slice(&c.tx_id.to_bytes());
    
        let kdf = Hkdf::<Sha256>::new(None, &cmk_bytes);
        let mut dek = [0u8; 32];
        kdf.expand(&info, &mut dek).map_err(|_| error!(CustomError::HkdfExpandFailed))?;
    
        emit!(DekResponse {
            chat: c.key(),
            chat_id: c.id,
            dek: dek,
        });
    
        Ok(())
    }

    pub fn update_status(
        ctx: Context<UpdateChatStatus>,
        new_status: u8,                   // e.g. STATUS_DONE or STATUS_ERROR
    ) -> Result<()> {
        let caller_key = ctx.accounts.caller.key();
        let c = &mut ctx.accounts.chat;
        let is_user = caller_key == c.user;
        let is_oracle = caller_key == ORACLE_IDENTITY;
        require!(is_user || is_oracle, CustomError::Unauthorized);
     
        require!(
            new_status == STATUS_DONE || new_status == STATUS_ERROR || new_status == STATUS_PENDING,
            CustomError::Unauthorized
        );
        c.status = new_status;
    
        emit!(StatusChanged {
            chat: c.key(),
            chat_id: c.id,
            status: c.status,
        });
        Ok(())
    }

    pub fn delegate_chat(ctx: Context<DelegateChat>, chat_id: u64) -> Result<()> {
        ctx.accounts.delegate_chat(
            &ctx.accounts.payer,
            &[
                CHAT_SEED,
                ctx.accounts.context_account.key().as_ref(),
                &chat_id.to_le_bytes(),
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

#[derive(Accounts)]
#[instruction(chat_id: u64)]
pub struct CreateChat<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    // MUST be the owner; ensures "context uniquely connected to the user"
    #[account(
        mut,
        constraint = context_account.owner == payer.key() @ CustomError::ContextOwnerMismatch
    )]
    pub context_account: Account<'info, ContextAccount>,

    /// creates the interaction PDA if needed
    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + Chat::INIT_SPACE,
        seeds = [
            CHAT_SEED,
            context_account.key().as_ref(),
            &chat_id.to_le_bytes(),
        ],
        bump
    )]
    pub chat: Account<'info, Chat>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct GetDek<'info> {
    /// chat.user OR the oracle identity.
    #[account(mut)]
    pub caller: Signer<'info>,

    /// Must be owned by this program.
    #[account(mut)]
    pub chat: Account<'info, Chat>,
}

#[delegate]
#[derive(Accounts)]
#[instruction(chat_id: u64)]
pub struct DelegateChat<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: the correct chat account
    #[account(
        mut, del,
        seeds = [CHAT_SEED, context_account.key().as_ref(), &chat_id.to_le_bytes()],
        bump
    )]
    /// CHECK: we only use seeds + delegation, no need to deserialize here
    pub chat: AccountInfo<'info>,

    /// CHECK: we accept any context
    pub context_account: AccountInfo<'info>,
}

#[commit]
#[derive(Accounts)]
#[instruction(chat_id: u64)]
pub struct UndelegateChat<'info> {
    /// CHECK: Matched against the chat account
    pub user: AccountInfo<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [CHAT_SEED, context_account.key().as_ref(), &chat_id.to_le_bytes()],
        bump
    )]
    pub chat: Account<'info, Chat>,

    /// CHECK: we accept any context
    pub context_account: AccountInfo<'info>,
}

/// Accounts

#[account]
pub struct ContextAccount {
    pub owner: Pubkey,
    pub next_chat_id: u64,
}

impl ContextAccount { pub fn seed() -> &'static [u8] { b"context" } }

#[account]
#[derive(InitSpace)]
pub struct Chat {
    /// ---- fixed-size header (stable offsets) ----
    pub context: Pubkey,
    pub user: Pubkey,
    pub id: u64,
    pub created_at: i64, // unix timestamp
    pub status: u8,
    pub cmk: Pubkey,
    pub tx_id: Pubkey,
}

#[derive(Accounts)]
pub struct UpdateChatStatus<'info> {
    /// chat.user OR the oracle identity.
    #[account(mut)]
    pub caller: Signer<'info>,

    #[account(mut)]
    pub chat: Account<'info, Chat>,
}

#[account]
pub struct Identity {}
