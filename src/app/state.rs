#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Dashboard,
    Rpc,
    Config,
}

#[derive(Debug, Default)]
pub struct State {
    pub active_tab: Tab,
}
