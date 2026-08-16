use crate::account_auth_middleware::AccountAuthenticated;
use crate::crypto;
use crate::db;
use crate::error::ApiError;
use crate::models::{Account, AuthenticatedAccount, LoginAccount, RegisterAccount};
use crate::validators::{valid_email, valid_password};
use actix_web::{get, post, web, Error, HttpResponse};
use chrono::{Duration, Utc};
use deadpool_postgres::{Client, Pool};

/// Session tokens are long-lived (30 days) - this is a bearer token for a JS SPA + RuneLite
/// plugin, not a browser cookie, so there is no silent renewal-on-visit; a full re-login is
/// the renewal path.
const SESSION_TTL: Duration = Duration::days(30);

async fn issue_session(client: &Client, account_id: i64) -> Result<String, ApiError> {
    let token = crypto::new_session_token();
    let token_hash = crypto::session_token_hash(&token);
    let expires_at = Utc::now() + SESSION_TTL;
    db::create_account_session(client, account_id, &token_hash, &expires_at).await?;
    Ok(token)
}

#[post("/register")]
pub async fn register(
    register_account: web::Json<RegisterAccount>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let email = register_account.email.trim().to_string();
    if !valid_email(&email) {
        return Ok(HttpResponse::BadRequest().body("Provided email is not valid"));
    }
    if !valid_password(&register_account.password) {
        return Ok(HttpResponse::BadRequest().body("Password must be between 8 and 256 characters"));
    }

    let password_hash = crypto::hash_password(&register_account.password)
        .map_err(|_| ApiError::InvalidCredentialsError)?;

    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let account_id = match db::create_account(&client, &email, &password_hash).await {
        Ok(account_id) => account_id,
        Err(ApiError::EmailAlreadyRegisteredError) => {
            return Ok(HttpResponse::Conflict().body("Email already registered"));
        }
        Err(err) => return Err(err.into()),
    };

    let token = issue_session(&client, account_id).await?;
    let account = db::get_account_by_email(&client, &email)
        .await?
        .ok_or(ApiError::InvalidCredentialsError)?;

    Ok(HttpResponse::Created().json(AuthenticatedAccount {
        account: Account {
            id: account.id,
            email: account.email,
            created_at: account.created_at,
        },
        token,
    }))
}

#[post("/login")]
pub async fn login(
    login_account: web::Json<LoginAccount>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let email = login_account.email.trim().to_string();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;

    let account = db::get_account_by_email(&client, &email).await?;
    let Some(account) = account else {
        return Err(ApiError::InvalidCredentialsError.into());
    };
    let Some(password_hash) = account.password_hash.as_deref() else {
        return Err(ApiError::InvalidCredentialsError.into());
    };
    if !crypto::verify_password(&login_account.password, password_hash) {
        return Err(ApiError::InvalidCredentialsError.into());
    }
    if account.disabled {
        return Err(ApiError::AccountDisabledError.into());
    }

    let token = issue_session(&client, account.id).await?;
    Ok(HttpResponse::Ok().json(AuthenticatedAccount {
        account: Account {
            id: account.id,
            email: account.email,
            created_at: account.created_at,
        },
        token,
    }))
}

#[get("/me")]
pub async fn me(authenticated: AccountAuthenticated) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().json(Account {
        id: authenticated.id,
        email: authenticated.email.clone(),
        created_at: authenticated.created_at,
    }))
}
