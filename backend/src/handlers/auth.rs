use actix_web::{HttpResponse, Responder, post, web};
use argon2::{
    Argon2, PasswordHasher,
    password_hash::{SaltString, rand_core::OsRng},
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct AuthPayload {
    pub wallet_address: String,
    pub signature: String,
}

#[derive(Deserialize)]
pub struct SignupPayload {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct SignupResponse {
    pub user_id: Uuid,
    pub username: String,
    pub allocated_public_key: String,
}

#[post("/signup")]
pub async fn signup(
    payload: web::Json<SignupPayload>,
    app_state: web::Data<crate::state::AppState>,
) -> impl Responder {
    let mut tx = match app_state.db.begin().await {
        Ok(t) => t,
        Err(_) => {
            return HttpResponse::InternalServerError()
                .json("Failed to initialize database transaction");
        }
    };

    let user_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1 OR  username = $2)",
    )
    .bind(&payload.email)
    .bind(&payload.username)
    .fetch_one(&mut *tx)
    .await
    .unwrap_or(false);

    if user_exists {
        return HttpResponse::BadRequest().json("Username or Email already registered");
    }

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = match argon2.hash_password(payload.password.as_bytes(), &salt) {
        Ok(h) => h.to_string(),
        Err(_) => return HttpResponse::InternalServerError().json("Password encryption failure"),
    };

    let allocated_key = sqlx::query_scalar::<_, String>(
        "SELECT public_key FROM mpc_accounts_pool WHERE user_id IS NULL LIMIT 1 FOR UPDATE SKIP LOCKED"
    ).fetch_optional(&mut *tx).await;

    let public_key = match allocated_key {
        Ok(Some(key)) => key,
        Ok(None) => {
            return HttpResponse::ServiceUnavailable()
                .json("Registration pool empty. No pre-allocated MPC keys available.");
        }
        Err(_) => {
            return HttpResponse::InternalServerError().json("Database error pulling pool key");
        }
    };

    let user_id = Uuid::new_v4();
    if let Err(_) = sqlx::query(
        "INSERT INTO users (id, username, email, password_hash) VALUES ($1, $2, $3, $4)",
    )
    .bind(&user_id)
    .bind(&payload.username)
    .bind(&payload.email)
    .bind(&password_hash)
    .execute(&mut *tx)
    .await
    {
        return HttpResponse::InternalServerError().json("Failed to save user instance");
    }

    if let Err(_) = sqlx::query(
        "UPDATE mpc_accounts_pool SET user_id = $1, assigned_at = NOW() WHERE public_key = $2",
    )
    .bind(&user_id)
    .bind(&public_key)
    .execute(&mut *tx)
    .await
    {
        return HttpResponse::InternalServerError().json("Failed to allocate pool context mapping");
    }

    if let Err(_) = sqlx::query(
        "INSERT INTO collateral (user_id, available_balance, locked_balance) VALUES ($1, 0.0, 0.0)",
    )
    .bind(&user_id)
    .execute(&mut *tx)
    .await
    {
        return HttpResponse::InternalServerError().json("Failed to create collateral records");
    }

    if let Err(_) = tx.commit().await {
        return HttpResponse::InternalServerError()
            .json("Failed to lock atomic transaction changes");
    }

    let node_urls = vec![
        (
            "Node 1",
            "postgres://mpc_operator_1:supersecurepasswordnode1@127.0.0.1:6431/postgres",
        ),
        (
            "Node 2",
            "postgres://mpc_operator_2:supersecurepasswordnode2@127.0.0.1:6432/postgres",
        ),
        (
            "Node 3",
            "postgres://mpc_operator_3:supersecurepasswordnode3@127.0.0.1:6433/postgres",
        ),
    ];

    for (node_name, url) in node_urls {
        match sqlx::PgPool::connect(url).await {
            Ok(pool) => {
                let res = sqlx::query(
                    "UPDATE mpc_shares SET user_id = $1, username = $2, email = $3, assigned_at = NOW() WHERE public_key = $4"
                )
                .bind(&user_id)
                .bind(&payload.username)
                .bind(&payload.email)
                .bind(&public_key)
                .execute(&pool)
                .await;

                if res.is_ok() {
                    println!("✨ Successfully synchronized metadata to {}", node_name);
                } else {
                    eprintln!("⚠️ Failed to execute query on {}", node_name);
                }
            }
            Err(e) => {
                eprintln!("❌ Network propagation failed for {}: {}", node_name, e);
            }
        }
    }

    HttpResponse::Ok().json(SignupResponse {
        user_id,
        username: payload.username.clone(),
        allocated_public_key: public_key,
    })
}

#[post("/signin")]
pub async fn signin(payload: web::Json<AuthPayload>) -> impl Responder {
    // TODO: validate signature and issue JWT
    HttpResponse::Ok().json("User authenticated")
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(signup).service(signin);
}
