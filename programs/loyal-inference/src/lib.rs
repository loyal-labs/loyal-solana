use anchor_lang::prelude::*;

declare_id!("56wz9nvWpZE9t4GawPCMC7gHCaJKz53WsCWrasSCbiBJ");

#[program]
pub mod loyal_inference {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
