use crate::ui::Tab;

#[derive(Clone, Debug)]
pub enum Command {
    // App
    Quit,
    SwitchTab(Tab),
    NextTab,
    PrevTab,
    Refresh,
    CycleMode,
    ToggleAutoUpdate,
    ToggleSearch,
    CycleSort,

    // Navigation
    MoveUp,
    MoveDown,
    PageUp,
    PageDown,
    Home,
    End,

    // Proxy
    ProxySelect,
    ProxyPrev,
    ProxyNext,
    ProxySpeedTest,
    ProxySpeedTestAll,

    // Provider
    ProviderToggleExpand,
    ProviderAdd,
    ProviderEdit,
    ProviderDelete,
    ProviderUpdate,
    ProviderUpdateAll,
    ProviderHealthCheck,

    // Connection
    ConnClose,
    ConnCloseAll,

    // Log
    LogClear,

    // Popup / Form
    Confirm,
    Cancel,

    // Form input
    FormNextField,
    FormPrevField,
    FormChar(char),
    FormBackspace,
    FormDelete,
    FormLeft,
    FormRight,
    FormSubmit,

    // Search input
    SearchChar(char),
    SearchBackspace,
    SearchCancel,

    #[allow(dead_code)]
    Noop,
}
