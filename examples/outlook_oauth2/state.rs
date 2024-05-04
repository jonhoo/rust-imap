use oauth2::basic::{BasicErrorResponseType, BasicTokenType};
use oauth2::{
    Client, EmptyExtraTokenFields, RevocationErrorResponseType, StandardErrorResponse,
    StandardRevocableToken, StandardTokenIntrospectionResponse, StandardTokenResponse,
};

pub(crate) type OauthClient = Client<
    StandardErrorResponse<BasicErrorResponseType>,
    StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>,
    BasicTokenType,
    StandardTokenIntrospectionResponse<EmptyExtraTokenFields, BasicTokenType>,
    StandardRevocableToken,
    StandardErrorResponse<RevocationErrorResponseType>,
>;

pub(crate) struct AppState {
    pub(crate) oauth_client: OauthClient,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            oauth_client: super::utils::build_oauth_client(),
        }
    }
}
