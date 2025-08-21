use bolt_lang::*;

declare_id!("A2F8ZzUpd7wXEqYTfxKUoCxtTf21uGc971QL8AhxcAFh");

#[component]
pub struct Chat {
    pub msg: u64,
    pub state: ChatState,
    pub owner: Option<Pubkey>,
}

#[component_deserialize]
#[derive(PartialEq)]
pub enum ChatState {
    UserTurn,
    ModelTurn,
}

impl Default for Chat {
    fn default() -> Self {
        Self::new(ChatInit {
            msg: 0,
            state: ChatState::UserTurn,
            owner: None,
        })
    }
}