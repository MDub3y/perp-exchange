use sqlx::postgres::PgPool;

pub struct AppState {
    pub db: PgPool,
    pub app_name: String,
}

impl AppState {
    pub fn new(db_pool: PgPool) -> Self {
        Self {
            db: db_pool,
            app_name: "Perp-CLOB-Exchange".to_string(),
        }
    }
}
