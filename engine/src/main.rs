use engine::ExecuteEngine;
use redis::RedisManager;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let redis_manager = RedisManager::new()
        .await
        .expect("Failed to initialize engine message spine");
    let mut engine = ExecuteEngine::new(redis_manager).await;
    engine.start_polling_loop().await;
}
