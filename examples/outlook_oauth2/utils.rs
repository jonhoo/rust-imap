use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::url::Url;
use oauth2::AccessToken;
use oauth2::TokenResponse;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope, TokenUrl,
};
use std::env;

pub(crate) fn build_auth_url(client: &super::state::OauthClient) -> Url {
    let (url, _csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(
            "https://outlook.office.com/IMAP.AccessAsUser.All".to_string(),
        ))
        .url();
    url
}

pub(crate) fn build_oauth_client() -> super::state::OauthClient {
    let client_id = ClientId::new(env::var("AZURE_OAUTH_APP_CLIENT_ID").unwrap());
    let client_secret = ClientSecret::new(env::var("AZURE_OAUTH_APP_CLIENT_SECRET").unwrap());
    let tenant_id = env::var("AZURE_OAUTH_APP_TENANT_ID").unwrap();
    let auth_url = AuthUrl::new(format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/authorize",
        tenant_id
    ))
    .expect("valid url");
    let token_url = TokenUrl::new(format!(
        "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
        tenant_id
    ))
    .expect("valid url");
    BasicClient::new(client_id, Some(client_secret), auth_url, Some(token_url)).set_redirect_uri(
        RedirectUrl::new("https://localhost:3993".to_string()).expect("valid url"),
    )
}

pub(crate) async fn exchange_code_for_token(
    client: &super::state::OauthClient,
    code: String,
) -> AccessToken {
    client
        .exchange_code(AuthorizationCode::new(code))
        .request_async(async_http_client)
        .await
        .expect("response with token")
        .access_token()
        .to_owned()
}
