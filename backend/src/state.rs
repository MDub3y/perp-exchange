pub struct AppState {
    pub app_name: String,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            app_name: "Perp-CLOB-Exchange".to_string(),
        }
    }
}
