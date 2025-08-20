use anchor_lang::prelude::*;

declare_id!("EuRoGk754ioCtmTaF4BTqtrcVnwkWSKLrUnKiQEdu55P");

#[program]
pub mod loyal {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
