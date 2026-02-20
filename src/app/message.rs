use crate::app::state::Tab;

#[derive(Debug, Clone, Copy)]
pub enum Message {
    SelectTab(Tab),
}
