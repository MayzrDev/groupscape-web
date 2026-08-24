use crate::account_auth_middleware::AccountAuthenticated;
use crate::character_auth_middleware::CharacterAuthenticationCache;
use crate::config::Config;
use crate::crypto;
use crate::db;
use crate::discord;
use crate::error::ApiError;
use crate::models::{
    Account, AccountApiKey, AuthenticatedAccount, ChangeAccountPassword, Character,
    CharacterGroupLink, DiscordCallbackQuery, LinkCharacter, LinkCharacterToGroup, LoginAccount,
    RegisterAccount, UpdateAccountUsername,
};
use crate::validators::{valid_name, valid_password};
use actix_web::{delete, get, post, put, web, Error, HttpRequest, HttpResponse};
use chrono::{Duration, Utc};
use deadpool_postgres::{Client, Pool};

/// Session tokens are long-lived (30 days) - this is a bearer token for a JS SPA + RuneLite
/// plugin, not a browser cookie, so there is no silent renewal-on-visit; a full re-login is
/// the renewal path.
const SESSION_TTL: Duration = Duration::days(30);

fn request_ip(req: &HttpRequest) -> Option<String> {
    req.connection_info()
        .realip_remote_addr()
        .map(|s| s.to_string())
}

fn request_user_agent(req: &HttpRequest) -> Option<String> {
    req.headers()
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

async fn issue_session(client: &Client, account_id: i64, req: &HttpRequest) -> Result<String, ApiError> {
    let token = crypto::new_session_token();
    let token_hash = crypto::session_token_hash(&token);
    let expires_at = Utc::now() + SESSION_TTL;
    db::create_account_session_with_meta(
        client,
        account_id,
        &token_hash,
        &expires_at,
        request_ip(req).as_deref(),
        request_user_agent(req).as_deref(),
    )
    .await?;
    Ok(token)
}

#[post("/register")]
pub async fn register(
    register_account: web::Json<RegisterAccount>,
    db_pool: web::Data<Pool>,
    req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let username = register_account.username.trim().to_string();
    if !valid_name(&username) {
        return Ok(HttpResponse::BadRequest().body("Provided username is not valid"));
    }
    if !valid_password(&register_account.password) {
        return Ok(HttpResponse::BadRequest().body("Password must be between 8 and 256 characters"));
    }

    let password_hash = crypto::hash_password(&register_account.password)
        .map_err(|_| ApiError::InvalidCredentialsError)?;

    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let account_id = match db::create_account(&client, &username, &password_hash).await {
        Ok(account_id) => account_id,
        Err(ApiError::UsernameAlreadyRegisteredError) => {
            return Ok(HttpResponse::Conflict().body("Username already registered"));
        }
        Err(err) => return Err(err.into()),
    };

    let token = issue_session(&client, account_id, &req).await?;
    let api_key = crypto::new_api_key();
    db::set_account_api_key_hash(&client, account_id, &crypto::api_key_hash(&api_key)).await?;
    let account = db::get_account_by_username(&client, &username)
        .await?
        .ok_or(ApiError::InvalidCredentialsError)?;

    Ok(HttpResponse::Created().json(AuthenticatedAccount {
        account: Account {
            id: account.id,
            username: account.username,
            discord_name: account.discord_name,
            created_at: account.created_at,
            must_change_password: account.must_change_password,
        },
        token,
        api_key: Some(api_key),
    }))
}

#[post("/login")]
pub async fn login(
    login_account: web::Json<LoginAccount>,
    db_pool: web::Data<Pool>,
    req: HttpRequest,
) -> Result<HttpResponse, Error> {
    let username = login_account.username.trim().to_string();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;

    let account = db::get_account_by_username(&client, &username).await?;
    let Some(account) = account else {
        return Err(ApiError::InvalidCredentialsError.into());
    };

    // Lockout check happens before the password comparison so an already-locked account
    // doesn't leak whether the presented password happens to be correct.
    if account
        .locked_until
        .is_some_and(|locked_until| locked_until > Utc::now())
    {
        return Err(ApiError::AccountLockedError.into());
    }

    let Some(password_hash) = account.password_hash.as_deref() else {
        return Err(ApiError::InvalidCredentialsError.into());
    };
    if !crypto::verify_password(&login_account.password, password_hash) {
        let locked_until = db::record_failed_login(&client, account.id).await?;
        return match locked_until {
            Some(locked_until) if locked_until > Utc::now() => {
                Err(ApiError::AccountLockedError.into())
            }
            _ => Err(ApiError::InvalidCredentialsError.into()),
        };
    }
    if account.status != "active" {
        return Err(ApiError::AccountDisabledError.into());
    }

    db::reset_login_lockout_and_record_login(&client, account.id).await?;
    let token = issue_session(&client, account.id, &req).await?;
    Ok(HttpResponse::Ok().json(AuthenticatedAccount {
        account: Account {
            id: account.id,
            username: account.username,
            discord_name: account.discord_name,
            created_at: account.created_at,
            must_change_password: account.must_change_password,
        },
        token,
        api_key: None,
    }))
}

#[get("/me")]
pub async fn me(authenticated: AccountAuthenticated) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().json(Account {
        id: authenticated.id,
        username: authenticated.username.clone(),
        discord_name: authenticated.discord_name.clone(),
        created_at: authenticated.created_at,
        must_change_password: authenticated.must_change_password,
    }))
}

/// No password re-entry required - the bearer session token is already this API's proof of
/// identity for every other account mutation (linking characters, etc.), so username is no
/// different. Uniqueness is enforced the same way as `register`: a duplicate surfaces as a
/// Postgres unique-violation via `db::update_account_username`.
#[put("/username")]
pub async fn update_username(
    body: web::Json<UpdateAccountUsername>,
    authenticated: AccountAuthenticated,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let username = body.username.trim().to_string();
    if !valid_name(&username) {
        return Ok(HttpResponse::BadRequest().body("Provided username is not valid"));
    }

    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    match db::update_account_username(&client, authenticated.id, &username).await {
        Ok(()) => {}
        Err(ApiError::UsernameAlreadyRegisteredError) => {
            return Ok(HttpResponse::Conflict().body("Username already registered"));
        }
        Err(err) => return Err(err.into()),
    }

    Ok(HttpResponse::Ok().json(Account {
        id: authenticated.id,
        username: Some(username),
        discord_name: authenticated.discord_name.clone(),
        created_at: authenticated.created_at,
        must_change_password: authenticated.must_change_password,
    }))
}

/// No current-password re-entry required - the bearer session token is already this API's proof
/// of identity for every other account mutation, same reasoning as `update_username` above. Also
/// the escape hatch for `must_change_password`: a successful change here clears the flag,
/// whether it was set by an admin-triggered reset or is just a routine change.
#[put("/password")]
pub async fn change_password(
    body: web::Json<ChangeAccountPassword>,
    authenticated: AccountAuthenticated,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    if !valid_password(&body.new_password) {
        return Ok(HttpResponse::BadRequest().body("Password must be between 8 and 256 characters"));
    }

    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let new_password_hash =
        crypto::hash_password(&body.new_password).map_err(|_| ApiError::InvalidCredentialsError)?;
    db::update_account_password(&client, authenticated.id, &new_password_hash).await?;
    db::clear_must_change_password(&client, authenticated.id).await?;

    Ok(HttpResponse::NoContent().finish())
}

/// Hard delete, matching `groupscape-old`. No password re-confirmation - same bearer-token
/// trust level as every other account mutation on this API. Session/character/link cleanup
/// happens via `ON DELETE CASCADE` in `db::delete_account`.
#[delete("")]
pub async fn delete_account(
    authenticated: AccountAuthenticated,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    db::delete_account(&client, authenticated.id).await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Lists the authenticated account's linked characters, oldest-linked first — feeds the site's
/// character management flow (`site: link-character flow`).
#[get("/characters")]
pub async fn list_characters(
    authenticated: AccountAuthenticated,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let characters =
        db::list_characters_for_account_with_group_status(&client, authenticated.id).await?;
    let characters: Vec<Character> = characters.into_iter().map(Character::from).collect();
    Ok(HttpResponse::Ok().json(characters))
}

/// Unlinks a character from the authenticated account. `db::delete_character` relies on
/// `character_group_links`' `ON DELETE CASCADE` to also drop any group membership - no
/// separate unlink-from-group step needed, unlike `groupscape-old`.
#[delete("/characters/{character_id}")]
pub async fn unlink_character(
    path: web::Path<i64>,
    authenticated: AccountAuthenticated,
    db_pool: web::Data<Pool>,
    character_auth_cache: web::Data<CharacterAuthenticationCache>,
) -> Result<HttpResponse, Error> {
    let character_id = path.into_inner();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;

    let character = db::find_character_by_id(&client, character_id).await?;
    let character = match character {
        Some(character) if character.account_id == authenticated.id => character,
        _ => return Err(ApiError::CharacterNotFoundError.into()),
    };

    db::delete_character(&client, character_id).await?;
    character_auth_cache.invalidate(&character.account_hash);
    Ok(HttpResponse::NoContent().finish())
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
        let refreshed = db::update_character_display_rsn(&client, existing.id, &rsn, None, None).await?;
        return Ok(HttpResponse::Ok().json(Character::from(refreshed)));
    }

    if db::is_character_denylisted(&client, authenticated.id, &account_hash).await? {
        return Err(ApiError::CharacterLinkedToAnotherAccountError.into());
    }

    let character_count = db::count_characters_for_account(&client, authenticated.id).await?;
    if character_count >= db::CHARACTER_CAP_PER_ACCOUNT {
        return Err(ApiError::CharacterCapReachedError.into());
    }

    let character = db::create_character(&client, authenticated.id, &account_hash, &rsn).await?;
    Ok(HttpResponse::Created().json(Character::from(character)))
}

/// Generates a fresh API key and overwrites the stored hash immediately - no grace period, so
/// any plugin still holding the old key stops authenticating the moment this returns.
#[post("/api-key")]
pub async fn regenerate_api_key(
    authenticated: AccountAuthenticated,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let api_key = crypto::new_api_key();
    db::set_account_api_key_hash(&client, authenticated.id, &crypto::api_key_hash(&api_key))
        .await?;
    Ok(HttpResponse::Ok().json(AccountApiKey { api_key }))
}

/// Confirms a pending character (auto-created by the plugin's first telemetry request under a
/// given account_hash) - only after this can it be assigned to a group.
#[post("/characters/{character_id}/confirm")]
pub async fn confirm_character(
    path: web::Path<i64>,
    authenticated: AccountAuthenticated,
    db_pool: web::Data<Pool>,
    character_auth_cache: web::Data<CharacterAuthenticationCache>,
) -> Result<HttpResponse, Error> {
    let character_id = path.into_inner();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;

    let character = db::find_character_by_id(&client, character_id).await?;
    match character {
        Some(character) if character.account_id == authenticated.id && character.status == "pending" => {}
        _ => return Err(ApiError::CharacterNotFoundError.into()),
    }

    let confirmed = db::confirm_character(&client, character_id).await?;
    character_auth_cache.invalidate(&confirmed.account_hash);
    Ok(HttpResponse::Ok().json(Character::from(confirmed)))
}

/// Removes a *pending* character and permanently denylists its account_hash from re-linking to
/// this account - distinct from `unlink_character`, which only removes an already-confirmed
/// character and does not denylist.
#[delete("/characters/{character_id}/pending")]
pub async fn remove_pending_character(
    path: web::Path<i64>,
    authenticated: AccountAuthenticated,
    db_pool: web::Data<Pool>,
    character_auth_cache: web::Data<CharacterAuthenticationCache>,
) -> Result<HttpResponse, Error> {
    let character_id = path.into_inner();
    let mut client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;

    let character = db::find_character_by_id(&client, character_id).await?;
    let character = match character {
        Some(character) if character.account_id == authenticated.id && character.status == "pending" => {
            character
        }
        _ => return Err(ApiError::CharacterNotFoundError.into()),
    };

    db::remove_pending_character(
        &mut client,
        authenticated.id,
        character_id,
        &character.account_hash,
    )
    .await?;
    character_auth_cache.invalidate(&character.account_hash);
    Ok(HttpResponse::NoContent().finish())
}

/// The confirm card's 3D portrait needs to render before a character has a group (it may still
/// be pending), so this is keyed by ownership of the character itself rather than group
/// membership - reads from `character_mesh`, populated by the plugin's account-hash-scoped
/// portrait upload (`authed::update_character_portrait`).
#[get("/characters/{character_id}/portrait")]
pub async fn get_character_portrait(
    path: web::Path<i64>,
    authenticated: AccountAuthenticated,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let character_id = path.into_inner();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;

    let character = db::find_character_by_id(&client, character_id).await?;
    match character {
        Some(character) if character.account_id == authenticated.id => {}
        _ => return Err(ApiError::CharacterNotFoundError.into()),
    }

    let mesh = db::get_character_mesh(&client, character_id).await?;
    match mesh {
        Some(mesh) => Ok(HttpResponse::Ok()
            .append_header(("Cache-Control", "private, max-age=60"))
            .content_type("application/octet-stream")
            .body(mesh)),
        None => Ok(HttpResponse::NotFound().finish()),
    }
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
    character_auth_cache: web::Data<CharacterAuthenticationCache>,
) -> Result<HttpResponse, Error> {
    let mut client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;

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

    let link =
        db::link_character_to_group(&mut client, character.id, authenticated.id, group_id).await?;
    character_auth_cache.invalidate(&character.account_hash);
    let response = HttpResponse::build(if already_linked_to_this_group {
        actix_web::http::StatusCode::OK
    } else {
        actix_web::http::StatusCode::CREATED
    })
    .json(CharacterGroupLink::from(link));
    Ok(response)
}

/// Lets the account owner leave a character's current group so it can be linked to a
/// different one afterward - `link_character_to_group` refuses to switch a character straight
/// from one group to another, so this is the explicit first step. No-ops (200) if the
/// character wasn't in a group to begin with.
#[post("/characters/{character_id}/leave-group")]
pub async fn leave_group(
    path: web::Path<i64>,
    authenticated: AccountAuthenticated,
    db_pool: web::Data<Pool>,
    character_auth_cache: web::Data<CharacterAuthenticationCache>,
) -> Result<HttpResponse, Error> {
    let character_id = path.into_inner();
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;

    let character = db::find_character_by_id(&client, character_id).await?;
    let character = match character {
        Some(character) if character.account_id == authenticated.id => character,
        _ => return Err(ApiError::CharacterNotFoundError.into()),
    };

    db::unlink_character_from_group(&client, character_id).await?;
    character_auth_cache.invalidate(&character.account_hash);
    Ok(HttpResponse::Ok().finish())
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
    req: HttpRequest,
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
    let (_, fresh_api_key) = match db::get_account_by_discord_id(&client, &discord_user.id).await? {
        Some(account) => (account, None),
        None => {
            let account_id =
                db::create_account_with_discord_id(&client, &discord_user.id).await?;
            let api_key = crypto::new_api_key();
            db::set_account_api_key_hash(&client, account_id, &crypto::api_key_hash(&api_key))
                .await?;
            let account = db::get_account_by_discord_id(&client, &discord_user.id)
                .await?
                .ok_or(ApiError::InvalidCredentialsError)?;
            (account, Some(api_key))
        }
    };
    db::update_account_discord_name(&client, &discord_user.id, &discord_user.name).await?;
    let account = db::get_account_by_discord_id(&client, &discord_user.id)
        .await?
        .ok_or(ApiError::InvalidCredentialsError)?;
    if account.status != "active" {
        return Ok(redirect_to("error=account_disabled"));
    }

    let token = issue_session(&client, account.id, &req).await?;
    let fragment = match fresh_api_key {
        Some(api_key) => format!("token={}&api_key={}", token, api_key),
        None => format!("token={}", token),
    };
    Ok(redirect_to(&fragment))
}
