use crate::account_auth_middleware::AccountAuthenticated;
use crate::config::{Config, PushConfig};
use crate::db;
use crate::error::ApiError;
use crate::models::{AlertEvent, SubscribePush, UnsubscribePush};
use actix_web::{delete, get, post, web, Error, HttpResponse};
use data_encoding::BASE64URL_NOPAD;
use deadpool_postgres::{Client, Pool};
use web_push_native::{
    jwt_simple::algorithms::ES256KeyPair, p256::PublicKey, Auth, WebPushBuilder,
};

/// Unauthed - the site needs this before the user has necessarily interacted with push at all,
/// to hand the browser's `PushManager.subscribe()` its `applicationServerKey`.
#[get("/push/vapid-public-key")]
pub async fn vapid_public_key(config: web::Data<Config>) -> Result<HttpResponse, Error> {
    if !config.push.enabled {
        return Err(ApiError::PushNotConfiguredError.into());
    }
    Ok(HttpResponse::Ok().json(serde_json::json!({ "publicKey": config.push.vapid_public_key })))
}

/// Upserts on `endpoint` conflict (see `db::upsert_push_subscription`) - re-subscribing (e.g. the
/// browser rotated its push keys) just refreshes the stored row rather than erroring.
#[post("/push/subscribe")]
pub async fn subscribe(
    body: web::Json<SubscribePush>,
    authenticated: AccountAuthenticated,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    let subscription = db::upsert_push_subscription(
        &client,
        authenticated.id,
        &body.endpoint,
        &body.keys.p256dh,
        &body.keys.auth,
    )
    .await?;
    Ok(HttpResponse::Created().json(serde_json::json!({ "id": subscription.id })))
}

#[delete("/push/subscribe")]
pub async fn unsubscribe(
    body: web::Json<UnsubscribePush>,
    authenticated: AccountAuthenticated,
    db_pool: web::Data<Pool>,
) -> Result<HttpResponse, Error> {
    let client: Client = db_pool.get().await.map_err(ApiError::PoolError)?;
    db::delete_push_subscription(&client, authenticated.id, &body.endpoint).await?;
    Ok(HttpResponse::NoContent().finish())
}

enum SendOutcome {
    Ok,
    Gone,
    Failed,
}

/// Fire-and-forget: looks up the account's push subscriptions and sends the alert to each,
/// pruning any subscription the push service reports as gone (404/410 - the browser unsubscribed
/// or the endpoint expired). Spawned off the caller rather than awaited, and every failure is
/// logged rather than propagated - a dead or misconfigured push setup must never fail the
/// plugin's telemetry upload that triggered it.
pub fn dispatch_alert_push(
    push_config: PushConfig,
    db_pool: Pool,
    account_id: i64,
    member_name: String,
    alert: AlertEvent,
) {
    if !push_config.enabled {
        return;
    }
    tokio::spawn(async move {
        let client: Client = match db_pool.get().await {
            Ok(client) => client,
            Err(err) => {
                log::warn!("push: failed to get db client: {}", err);
                return;
            }
        };
        let subscriptions = match db::list_push_subscriptions_for_account(&client, account_id).await
        {
            Ok(subscriptions) => subscriptions,
            Err(err) => {
                log::warn!("push: failed to list subscriptions: {}", err);
                return;
            }
        };
        drop(client);
        if subscriptions.is_empty() {
            return;
        }

        let (title, body) = alert.push_title_and_body(&member_name);
        let payload = serde_json::json!({
            "title": title,
            "body": body,
            "type": alert.alert_type(),
            "requireInteraction": alert.requires_interaction(),
        })
        .to_string();

        for subscription in subscriptions {
            let vapid_private_key = push_config.vapid_private_key.clone();
            let vapid_subject = push_config.vapid_subject.clone();
            let payload = payload.clone();
            let endpoint = subscription.endpoint.clone();
            let p256dh = subscription.p256dh.clone();
            let auth = subscription.auth.clone();

            let outcome = tokio::task::spawn_blocking(move || {
                send_push_sync(
                    &vapid_private_key,
                    &vapid_subject,
                    &endpoint,
                    &p256dh,
                    &auth,
                    &payload,
                )
            })
            .await
            .unwrap_or(SendOutcome::Failed);

            if let SendOutcome::Gone = outcome {
                if let Ok(client) = db_pool.get().await {
                    let _ =
                        db::delete_push_subscription_by_endpoint(&client, &subscription.endpoint)
                            .await;
                }
            }
        }
    });
}

/// Signs, encrypts (RFC8291 aes128gcm), and sends one push message via blocking `ureq`,
/// matching this codebase's existing sync-HTTP-in-`spawn_blocking` pattern (see
/// `discord::exchange_code`). `web_push_native` is a pure-Rust (`p256`/`aes-gcm`) implementation
/// rather than the `web-push` crate, which drags in `openssl-sys` transitively via `ece` - this
/// project standardizes on `rustls`/pure-Rust crypto everywhere else (see `ureq`'s `rustls`
/// feature), so this keeps that consistent.
fn send_push_sync(
    vapid_private_key: &str,
    vapid_subject: &str,
    endpoint: &str,
    p256dh: &str,
    auth: &str,
    payload: &str,
) -> SendOutcome {
    let endpoint_uri: http::Uri = match endpoint.parse() {
        Ok(uri) => uri,
        Err(err) => {
            log::warn!("push: invalid subscription endpoint: {}", err);
            return SendOutcome::Failed;
        }
    };
    let ua_public = match BASE64URL_NOPAD
        .decode(p256dh.as_bytes())
        .ok()
        .and_then(|bytes| PublicKey::from_sec1_bytes(&bytes).ok())
    {
        Some(key) => key,
        None => {
            log::warn!("push: invalid subscription p256dh key");
            return SendOutcome::Failed;
        }
    };
    let ua_auth_bytes = match BASE64URL_NOPAD.decode(auth.as_bytes()) {
        Ok(bytes) if bytes.len() == 16 => bytes,
        _ => {
            log::warn!("push: invalid subscription auth secret");
            return SendOutcome::Failed;
        }
    };
    let ua_auth = *Auth::from_slice(&ua_auth_bytes);

    let vapid_key_bytes = match BASE64URL_NOPAD.decode(vapid_private_key.as_bytes()) {
        Ok(bytes) => bytes,
        Err(err) => {
            log::warn!("push: invalid VAPID private key encoding: {}", err);
            return SendOutcome::Failed;
        }
    };
    let vapid_key_pair = match ES256KeyPair::from_bytes(&vapid_key_bytes) {
        Ok(kp) => kp,
        Err(err) => {
            log::warn!("push: invalid VAPID private key: {}", err);
            return SendOutcome::Failed;
        }
    };

    let request = WebPushBuilder::new(endpoint_uri, ua_public, ua_auth)
        .with_vapid(&vapid_key_pair, vapid_subject)
        .build(payload.as_bytes().to_vec());
    let request = match request {
        Ok(request) => request,
        Err(err) => {
            log::warn!("push: failed to build request: {}", err);
            return SendOutcome::Failed;
        }
    };

    match ureq::run(request) {
        Ok(_) => SendOutcome::Ok,
        Err(ureq::Error::StatusCode(404)) | Err(ureq::Error::StatusCode(410)) => SendOutcome::Gone,
        Err(err) => {
            log::warn!("push: send failed: {}", err);
            SendOutcome::Failed
        }
    }
}
