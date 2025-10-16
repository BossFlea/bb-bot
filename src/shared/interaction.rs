use crate::shared::menu::MenuMessage;

pub mod modal;

pub struct MenuChange<'a, T> {
    pub new_state: Option<T>,
    pub update_referrer_state: bool,
    pub message: MessageEdit<'a>,
}

impl<'a, T> MenuChange<'a, T> {
    pub fn new(new_state: T, message: MessageEdit<'a>) -> Self {
        Self {
            new_state: Some(new_state),
            update_referrer_state: false,
            message,
        }
    }
    pub fn update_state(new_state: T, message: MessageEdit<'a>) -> Self {
        Self {
            new_state: Some(new_state),
            update_referrer_state: true,
            message,
        }
    }
    pub fn message(message: MessageEdit<'a>) -> Self {
        Self {
            new_state: None,
            update_referrer_state: false,
            message,
        }
    }
    pub fn none() -> Self {
        Self {
            new_state: None,
            update_referrer_state: false,
            message: MessageEdit::NoEdit,
        }
    }
}

pub enum MessageEdit<'a> {
    Interaction(MenuMessage<'a>),
    Direct(MenuMessage<'a>),
    NoEdit,
}
