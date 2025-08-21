use bolt_lang::*;
use chat::{ChatState, Chat};

declare_id!("7SRQFFb81YCEuYg8R6XDwqfxzjgkwnYt6wJga3KCcM16");

pub const TEST_PDA_SEED: &[u8] = b"test-pda";

#[system]
pub mod inference_pay {

    pub fn execute(ctx: Context<Components>, args: Args) -> Result<Components> {
        let chat = &mut ctx.accounts.chat;

        // set up the transfer instruction
        let from = *ctx.accounts.from_pda.to_account_info();
        let to = *ctx.accounts.send_to.to_account_info();
        let amount = args.amount;

        let ix = anchor_lang::solana_program::system_instruction::transfer(&from.key, &to.key, amount);
        
        anchor_lang::solana_program::program::invoke_signed(
            &ix,
            &[
                from.clone(),
                to.clone(),
                ctx.accounts.program.to_account_info().clone(),
            ],
            &[&[TEST_PDA_SEED, &[ctx.bumps.get("test-pda").unwrap().clone()]]]
        )?;

        // update the chat state
        chat.msg += 1;
        if chat.state == ChatState::ModelTurn {
            chat.state = ChatState::UserTurn;
        }

        else {
            chat.state = ChatState::ModelTurn;
        }

        Ok(ctx.accounts)
    }

    #[system_input]
    pub struct Components {
        pub chat: Chat,
        pub from_pda: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub send_to: anchor_lang::solana_program::account_info::AccountInfo<'info>,
        pub program: anchor_lang::accounts::program::Program<'info, anchor_lang::system_program::System>
    }

    #[arguments]
    struct Args {
        pub amount: u64,
    }

}