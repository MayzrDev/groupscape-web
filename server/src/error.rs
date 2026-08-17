use actix_web::{HttpResponse, ResponseError};
use deadpool_postgres::PoolError;
use derive_more::{Display, From};

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Display, From)]
pub enum ApiError {
    PoolError(PoolError),
    PGError(tokio_postgres::error::Error),
    SerdeJsonError(serde_json::Error),
    #[from(ignore)]
    GroupCreationError(tokio_postgres::error::Error),
    #[from(ignore)]
    UpdateGroupMemberError(tokio_postgres::error::Error),
    #[from(ignore)]
    GetGroupError(tokio_postgres::error::Error),
    #[from(ignore)]
    AddMemberError(tokio_postgres::error::Error),
    #[from(ignore)]
    GetGroupDataError(tokio_postgres::error::Error),
    #[from(ignore)]
    DeleteGroupMemberError(tokio_postgres::error::Error),
    #[from(ignore)]
    RenameGroupError(tokio_postgres::error::Error),
    #[from(ignore)]
    IsMemberBlockedError(tokio_postgres::error::Error),
    #[from(ignore)]
    BlockGroupMemberError(tokio_postgres::error::Error),
    #[from(ignore)]
    UnblockGroupMemberError(tokio_postgres::error::Error),
    #[from(ignore)]
    GetBlockedMembersError(tokio_postgres::error::Error),
    MemberBlockedError,
    #[from(ignore)]
    RerollGroupTokenError(tokio_postgres::error::Error),
    #[from(ignore)]
    DeleteGroupError(tokio_postgres::error::Error),
    #[from(ignore)]
    IsMemberInGroupError(tokio_postgres::error::Error),
    #[from(ignore)]
    GetSkillsDataError(tokio_postgres::error::Error),
    #[from(ignore)]
    GetMemberColorsError(tokio_postgres::error::Error),
    #[from(ignore)]
    UpsertMemberMeshError(tokio_postgres::error::Error),
    #[from(ignore)]
    GetMemberMeshError(tokio_postgres::error::Error),
    GroupFullError,
    UreqError(ureq::Error),
    GroupMemberValidationError(String),
    #[from(ignore)]
    CreateAccountError(tokio_postgres::error::Error),
    EmailAlreadyRegisteredError,
    #[from(ignore)]
    GetAccountError(tokio_postgres::error::Error),
    #[from(ignore)]
    CreateAccountSessionError(tokio_postgres::error::Error),
    InvalidCredentialsError,
    AccountDisabledError,
    #[from(ignore)]
    #[display("{_0}: {_1}")]
    AdminDbError(String, tokio_postgres::error::Error),
    AdminNotFoundError,
    AdminRateLimitedError,
    #[from(ignore)]
    DiscordOAuthError(String),
    #[from(ignore)]
    CreateCharacterError(tokio_postgres::error::Error),
    #[from(ignore)]
    GetCharacterError(tokio_postgres::error::Error),
    #[from(ignore)]
    DeleteCharacterError(tokio_postgres::error::Error),
    CharacterLinkedToAnotherAccountError,
    CharacterCapReachedError,
    CharacterNotFoundError,
    CharacterAlreadyInGroupError,
    GroupNotFoundOrInvalidTokenError,
    #[from(ignore)]
    LinkCharacterToGroupError(tokio_postgres::error::Error),
    #[from(ignore)]
    GetCharacterGroupLinkError(tokio_postgres::error::Error),
    #[from(ignore)]
    GetGroupAdminError(tokio_postgres::error::Error),
    #[from(ignore)]
    UpdateAccountEmailError(tokio_postgres::error::Error),
    #[from(ignore)]
    UpdateAccountPasswordError(tokio_postgres::error::Error),
    #[from(ignore)]
    DeleteAccountError(tokio_postgres::error::Error),
    AccountHasNoPasswordSetError,
    IncorrectCurrentPasswordError,
    #[from(ignore)]
    GetGroupPermissionsError(tokio_postgres::error::Error),
    #[from(ignore)]
    UpdateGroupPermissionsError(tokio_postgres::error::Error),
    CannotModifyGroupAdminPermissionsError,
    AccountAuthRequiredError,
    PermissionDeniedError,
}
impl std::error::Error for ApiError {}
fn handle_pg_error(err: &tokio_postgres::error::Error, name: &str) -> HttpResponse {
    match err.as_db_error() {
        Some(db_error) => log::error!("{}: {}", name, db_error.message()),
        None => log::error!("{}: {}", name, err),
    };

    HttpResponse::InternalServerError().finish()
}
impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        match *self {
            ApiError::PoolError(ref err) => {
                log::error!("PoolError: {}", err);
                HttpResponse::InternalServerError().body(format!("PoolError: {}", err))
            }
            ApiError::GroupCreationError(ref err) => handle_pg_error(err, "GroupCreationError"),
            ApiError::UpdateGroupMemberError(ref err) => {
                handle_pg_error(err, "UpdateGroupMemberError")
            }
            ApiError::PGError(ref err) => handle_pg_error(err, "PGError"),
            ApiError::GetGroupError(ref err) => handle_pg_error(err, "GetGroupError"),
            ApiError::AddMemberError(ref err) => handle_pg_error(err, "AddMemberError"),
            ApiError::GetGroupDataError(ref err) => handle_pg_error(err, "GetGroupDataError"),
            ApiError::IsMemberInGroupError(ref err) => handle_pg_error(err, "IsMemberInGroupError"),
            ApiError::GetSkillsDataError(ref err) => handle_pg_error(err, "GetSkillsDataError"),
            ApiError::GetMemberColorsError(ref err) => handle_pg_error(err, "GetMemberColorsError"),
            ApiError::UpsertMemberMeshError(ref err) => {
                handle_pg_error(err, "UpsertMemberMeshError")
            }
            ApiError::GetMemberMeshError(ref err) => handle_pg_error(err, "GetMemberMeshError"),
            ApiError::DeleteGroupMemberError(ref err) => {
                handle_pg_error(err, "DeleteGroupMemberError")
            }
            ApiError::RenameGroupError(ref err) => handle_pg_error(err, "RenameGroupError"),
            ApiError::IsMemberBlockedError(ref err) => handle_pg_error(err, "IsMemberBlockedError"),
            ApiError::BlockGroupMemberError(ref err) => {
                handle_pg_error(err, "BlockGroupMemberError")
            }
            ApiError::UnblockGroupMemberError(ref err) => {
                handle_pg_error(err, "UnblockGroupMemberError")
            }
            ApiError::GetBlockedMembersError(ref err) => {
                handle_pg_error(err, "GetBlockedMembersError")
            }
            ApiError::MemberBlockedError => {
                HttpResponse::Forbidden().body("This player has been blocked from the group")
            }
            ApiError::RerollGroupTokenError(ref err) => {
                handle_pg_error(err, "RerollGroupTokenError")
            }
            ApiError::DeleteGroupError(ref err) => handle_pg_error(err, "DeleteGroupError"),
            ApiError::SerdeJsonError(ref err) => {
                log::error!("SerdeJsonError: {}", err);
                HttpResponse::InternalServerError().body(format!("SerdeJsonError: {}", err))
            }
            ApiError::GroupFullError => HttpResponse::BadRequest()
                .body("Group has already reached the maximum amount of players"),
            ApiError::UreqError(ref err) => {
                log::error!("UreqError: {}", err);
                HttpResponse::InternalServerError().body(format!("UreqError: {}", err))
            }
            ApiError::GroupMemberValidationError(ref reason) => {
                log::error!("Validation error: {}", reason);
                HttpResponse::BadRequest().body(reason.clone())
            }
            ApiError::AdminDbError(ref context, ref err) => handle_pg_error(err, context),
            ApiError::AdminNotFoundError => HttpResponse::NotFound().finish(),
            ApiError::AdminRateLimitedError => HttpResponse::TooManyRequests().finish(),
            ApiError::CreateAccountError(ref err) => handle_pg_error(err, "CreateAccountError"),
            ApiError::EmailAlreadyRegisteredError => {
                HttpResponse::Conflict().body("Email already registered")
            }
            ApiError::GetAccountError(ref err) => handle_pg_error(err, "GetAccountError"),
            ApiError::CreateAccountSessionError(ref err) => {
                handle_pg_error(err, "CreateAccountSessionError")
            }
            ApiError::InvalidCredentialsError => {
                HttpResponse::Unauthorized().body("Invalid email or password")
            }
            ApiError::AccountDisabledError => HttpResponse::Forbidden().body("Account is disabled"),
            ApiError::DiscordOAuthError(ref reason) => {
                log::error!("DiscordOAuthError: {}", reason);
                HttpResponse::BadGateway().body("Discord login failed")
            }
            ApiError::CreateCharacterError(ref err) => handle_pg_error(err, "CreateCharacterError"),
            ApiError::GetCharacterError(ref err) => handle_pg_error(err, "GetCharacterError"),
            ApiError::DeleteCharacterError(ref err) => handle_pg_error(err, "DeleteCharacterError"),
            ApiError::CharacterLinkedToAnotherAccountError => HttpResponse::Conflict()
                .body("Character already linked to another account. Unlink it there first."),
            ApiError::CharacterCapReachedError => {
                HttpResponse::Forbidden().body("Character cap reached")
            }
            ApiError::CharacterNotFoundError => {
                HttpResponse::NotFound().body("Character not found")
            }
            ApiError::CharacterAlreadyInGroupError => HttpResponse::Conflict()
                .body("Character already belongs to a group. Leave that group first."),
            ApiError::GroupNotFoundOrInvalidTokenError => {
                HttpResponse::Unauthorized().body("Group not found or token is invalid")
            }
            ApiError::LinkCharacterToGroupError(ref err) => {
                handle_pg_error(err, "LinkCharacterToGroupError")
            }
            ApiError::GetCharacterGroupLinkError(ref err) => {
                handle_pg_error(err, "GetCharacterGroupLinkError")
            }
            ApiError::GetGroupAdminError(ref err) => handle_pg_error(err, "GetGroupAdminError"),
            ApiError::UpdateAccountEmailError(ref err) => {
                handle_pg_error(err, "UpdateAccountEmailError")
            }
            ApiError::UpdateAccountPasswordError(ref err) => {
                handle_pg_error(err, "UpdateAccountPasswordError")
            }
            ApiError::DeleteAccountError(ref err) => handle_pg_error(err, "DeleteAccountError"),
            ApiError::AccountHasNoPasswordSetError => {
                HttpResponse::BadRequest().body("This account has no password set")
            }
            ApiError::IncorrectCurrentPasswordError => {
                HttpResponse::Unauthorized().body("Current password is incorrect")
            }
            ApiError::GetGroupPermissionsError(ref err) => {
                handle_pg_error(err, "GetGroupPermissionsError")
            }
            ApiError::UpdateGroupPermissionsError(ref err) => {
                handle_pg_error(err, "UpdateGroupPermissionsError")
            }
            ApiError::CannotModifyGroupAdminPermissionsError => HttpResponse::Conflict()
                .body("The group admin's permissions are implicit and cannot be changed"),
            ApiError::AccountAuthRequiredError => HttpResponse::Unauthorized()
                .body("This action requires a logged-in account (X-Account-Authorization)"),
            ApiError::PermissionDeniedError => {
                HttpResponse::Forbidden().body("You do not have permission to perform this action")
            }
        }
    }
}
