use std::env;

use actix_web::{HttpResponse, Responder, post, web};
use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct SignupPayload {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct SigninPayload {
    pub username_or_email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthTokenResponse {
    pub token: String,
    pub token_type: String,
}

#[derive(Serialize)]
pub struct JwtClaims {
    pub sub: String,
    pub username: String,
    pub exp: i64,
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
        ("Node 1", env::var("NODE_1_DB_URL").unwrap_or_default()),
        ("Node 2", env::var("NODE_2_DB_URL").unwrap_or_default()),
        ("Node 3", env::var("NODE_3_DB_URL").unwrap_or_default()),
    ];

    for (node_name, url) in node_urls {
        if url.is_empty() {
            eprintln!("Environment configuration missing for target {}", node_name);
            continue;
        }
        if let Ok(pool) = sqlx::PgPool::connect(&url).await {
            let _ = sqlx::query(
                "UPDATE mpc_shares SET user_id = $1, username = $2, email = $3, assigned_at = NOW() WHERE public_key = $4"
            )
            .bind(&user_id)
            .bind(&payload.username)
            .bind(&payload.email)
            .bind(&public_key)
            .execute(&pool)
            .await;
            println!("Successfully synchronized metadata to {}", node_name);
        }
    }

    HttpResponse::Ok().json(SignupResponse {
        user_id,
        username: payload.username.clone(),
        allocated_public_key: public_key,
    })
}

#[post("/signin")]
pub async fn signin(
    payload: web::Json<SigninPayload>,
    app_state: web::Data<crate::state::AppState>,
) -> impl Responder {
    let user_record = sqlx::query!(
        "SELECT id, username, password_hash FROM users WHERE username = $1 OR email = $1",
        payload.username_or_email
    )
    .fetch_optional(&app_state.db)
    .await;

    let user = match user_record {
        Ok(Some(u)) => u,
        Ok(None) => {
            return HttpResponse::Unauthorized().json("Invalid identifier credentials provided");
        }
        Err(_) => return HttpResponse::InternalServerError().json("Database auth parsing failure"),
    };

    let parsed_hash = match PasswordHash::new(&user.password_hash) {
        Ok(h) => h,
        Err(_) => {
            return HttpResponse::InternalServerError()
                .json("Error loading cryptographic credentials representation");
        }
    };

    if Argon2::default()
        .verify_password(payload.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return HttpResponse::Unauthorized().json("Invalid password entry credentials");
    }

    let jwt_secret = env::var("JWT_SECRET")
        .unwrap_or_else(|_| "default_fallback_secret_key_string_matrix_987".to_string());
    let expiration = Utc::now() + Duration::hours(24);

    let claims = JwtClaims {
        sub: user.id.to_string(),
        username: user.username,
        exp: expiration.timestamp(),
    };

    let token = match encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    ) {
        Ok(t) => t,
        Err(_) => {
            return HttpResponse::InternalServerError()
                .json("Failed to synthesize signed auth string");
        }
    };

    HttpResponse::Ok().json(AuthTokenResponse {
        token,
        token_type: "Bearer".to_string(),
    })
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(signup).service(signin);
}
