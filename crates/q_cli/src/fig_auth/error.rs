use aws_sdk_ssooidc::error::SdkError;
use aws_sdk_ssooidc::operation::create_token::CreateTokenError;
use aws_sdk_ssooidc::operation::register_client::RegisterClientError;
use aws_sdk_ssooidc::operation::start_device_authorization::StartDeviceAuthorizationError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Ssooidc(#[from] Box<aws_sdk_ssooidc::Error>),
    #[error(transparent)]
    SdkRegisterClient(#[from] SdkError<RegisterClientError>),
    #[error(transparent)]
    SdkCreateToken(#[from] SdkError<CreateTokenError>),
    #[error(transparent)]
    SdkStartDeviceAuthorization(#[from] SdkError<StartDeviceAuthorizationError>),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    TimeComponentRange(#[from] time::error::ComponentRange),
    #[error(transparent)]
    Directories(#[from] crate::fig_util::directories::DirectoryError),
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    #[error("Security error: {}", .0)]
    Security(String),
    #[error(transparent)]
    StringFromUtf8(#[from] std::string::FromUtf8Error),
    #[error(transparent)]
    StrFromUtf8(#[from] std::str::Utf8Error),
    #[error(transparent)]
    DbOpenError(#[from] crate::fig_settings::error::DbOpenError),
    #[error(transparent)]
    Setting(#[from] crate::fig_settings::Error),
    #[error("No token")]
    NoToken,
    #[error("OAuth state mismatch. Actual: {} | Expected: {}", .actual, .expected)]
    OAuthStateMismatch { actual: String, expected: String },
    #[error("Timeout waiting for authentication to complete")]
    OAuthTimeout,
    #[error("No code received on redirect")]
    OAuthMissingCode,
    #[error("OAuth error: {0}")]
    OAuthCustomError(String),
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;
