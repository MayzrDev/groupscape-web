use crate::account_auth_middleware::AccountAuthenticated;
use crate::config::Config;
use crate::crypto;
use crate::db;
use crate::discord;
use crate::error::ApiError;
use crate::models::{
    Account, AuthenticatedAccount, Character, CharacterGroupLink, DiscordCallbackQuery,
    LinkCharacter, LinkCharacterToGroup, LoginAccount, RegisterAccount,
};
use crate::validators::{valid_email, valid_name, valid_password};
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

/// account_hash -> account, ported from `groupscape-old`'s one-click link flow: the plugin
/// hands the browser its account hash and an RSN, and the browser's already-authenticated
/// session is the proof of which account it links to. Re-linking the same account_hash to the
/// same account is treated as an idempotent RSN refresh (the RSN can change via a name change
/// while the underlying game account, identified by its stable hash, stays the same).
#[post("/characters/link")]
pub async fn link_character(
    link_character: web::Json<LinkCharacter>,
    authenticated: AccountAuthenticated,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let account_hash = link_character.account_hash.trim().to_string();
    let rsn = link_character.rsn.trim().to_string();
    if account_hash.is_empty() {
        return Ok(HttpResponse::BadRequest().body("account_hash must not be empty"));
    }
    if !valid_name(&rsn) {
        return Ok(HttpResponse::BadRequest().body("Provided RSN is not valid"));
    }

    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let existing = db::find_character_by_account_hash(&client, &account_hash).await?;

    if let Some(existing) = existing {
        if existing.account_id != authenticated.id {
            return Err(ApiError::CharacterLinkedToAnotherAccountError.into());
        }
        let refreshed = db::update_character_display_rsn(&client, existing.id, &rsn).await?;
        return Ok(HttpResponse::Ok().json(Character::from(refreshed)));
    }

    let character_count = db::count_characters_for_account(&client, authenticated.id).await?;
    if character_count >= db::CHARACTER_CAP_PER_ACCOUNT {
        return Err(ApiError::CharacterCapReachedError.into());
    }

    let character = db::create_character(&client, authenticated.id, &account_hash, &rsn).await?;
    Ok(HttpResponse::Created().json(Character::from(character)))
}

/// Ported from `groupscape-old`'s `character_group_links` invariant: a character (an
/// account-linked RuneScape account) can join/own only a single group at a time. Group
/// credentials are verified from the request body rather than the `Authorization` header
/// since that header already carries this endpoint's account bearer token - same shape as
/// `unauthed::create_group`, which also takes group credentials in the body.
#[post("/characters/link-group")]
pub async fn link_character_to_group(
    link: web::Json<LinkCharacterToGroup>,
    authenticated: AccountAuthenticated,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;

    let character = db::find_character_by_id(&client, link.character_id).await?;
    let character = match character {
        Some(character) if character.account_id == authenticated.id => character,
        _ => return Err(ApiError::CharacterNotFoundError.into()),
    };

    let group_id = db::get_group(&client, &link.group_name, &link.group_token)
        .await
        .map_err(|_| ApiError::GroupNotFoundOrInvalidTokenError)?;

    let already_linked_to_this_group = db::find_character_group_link(&client, character.id)
        .await?
        .is_some_and(|existing| existing.group_id == group_id);

    let link = db::link_character_to_group(&client, character.id, group_id).await?;
    let response = HttpResponse::build(if already_linked_to_this_group {
        actix_web::http::StatusCode::OK
    } else {
        actix_web::http::StatusCode::CREATED
    })
    .json(CharacterGroupLink::from(link));
    Ok(response)
}

/// Sends the browser to Discord's consent screen. `state` is a signed, self-verifying value
/// (see `crypto::new_oauth_state`) rather than something stashed server-side, since this API
/// has no session store to stash a pending-OAuth nonce in between this redirect and the
/// callback below.
#[get("/discord/redirect")]
pub async fn discord_redirect(config: web::Data<Config>) -> Result<HttpResponse, Error> {
    if !config.discord.enabled {
        return Ok(HttpResponse::ServiceUnavailable().body("Discord login is not configured"));
    }

    let state = crypto::new_oauth_state();
    Ok(HttpResponse::Found()
        .append_header(("Location", discord::authorize_url(&config.discord, &state)))
        .finish())
}

/// §8: Discord OAuth, `identify` scope only - a standalone login method ported from
/// `groupscape-old`'s Slice 29 decision: a Discord id with no matching account auto-creates
/// one, same as any other OAuth-first app, not link-only. Since this API is bearer-token (no
/// browser session), the outcome is handed back to the SPA via a URL fragment on `web_origin`
/// rather than a cookie - fragments never reach the server on the next request, so the token
/// doesn't end up logged in server/proxy access logs.
#[get("/discord/callback")]
pub async fn discord_callback(
    query: web::Query<DiscordCallbackQuery>,
    config: web::Data<Config>,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    if !config.discord.enabled {
        return Ok(HttpResponse::ServiceUnavailable().body("Discord login is not configured"));
    }

    let redirect_to = |fragment: &str| {
        HttpResponse::Found()
            .append_header((
                "Location",
                format!("{}#{}", config.web_origin.trim_end_matches('/'), fragment),
            ))
            .finish()
    };

    let (Some(code), Some(state)) = (query.code.as_deref(), query.state.as_deref()) else {
        return Ok(redirect_to("error=discord_state_mismatch"));
    };
    if !crypto::verify_oauth_state(state) {
        return Ok(redirect_to("error=discord_state_mismatch"));
    }

    let discord_user = match discord::exchange_code(&config.discord, code).await {
        Ok(user) => user,
        Err(_) => return Ok(redirect_to("error=discord_failed")),
    };

    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let account = match db::get_account_by_discord_id(&client, &discord_user.id).await? {
        Some(account) => account,
        None => {
            db::create_account_with_discord_id(&client, &discord_user.id).await?;
            db::get_account_by_discord_id(&client, &discord_user.id)
                .await?
                .ok_or(ApiError::InvalidCredentialsError)?
        }
    };
    if account.disabled {
        return Ok(redirect_to("error=account_disabled"));
    }

    let token = issue_session(&client, account.id).await?;
    Ok(redirect_to(&format!("token={}", token)))
}
