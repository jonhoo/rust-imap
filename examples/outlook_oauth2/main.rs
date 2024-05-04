use axum::extract::{Query, State};
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use axum_server::tls_rustls::RustlsConfig;
use oauth2::AccessToken;
use serde::Deserialize;
use std::sync::Arc;
use std::{env, net::SocketAddr, path::PathBuf};

mod state;
mod utils;

struct OutlookOAuth2 {
    user: String,
    access_token: AccessToken,
}

impl imap::Authenticator for OutlookOAuth2 {
    type Response = String;
    #[allow(unused_variables)]
    fn process(&self, data: &[u8]) -> Self::Response {
        format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            self.user,
            self.access_token.secret()
        )
    }
}

#[tokio::main]
async fn main() {
    // load environment variables
    let this_example_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("outlook_oauth2");
    dotenvy::from_path(this_example_dir.join(".env")).ok();

    // load self-signed certificates
    let certs_dir = this_example_dir.join("certs");
    let config = RustlsConfig::from_pem_file(
        certs_dir.join("imap.local.crt"),
        certs_dir.join("imap.local.key"),
    )
    .await
    .unwrap();

    // define application and ...
    let state = Arc::new(state::AppState::default());
    let app = Router::new().route("/", get(home)).with_state(state);

    // ... launch it
    axum_server::bind_rustls(SocketAddr::from(([127, 0, 0, 1], 3993)), config)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Deserialize)]
struct AuthCode {
    code: String,
    state: String,
}

async fn home(
    auth_code: Option<Query<AuthCode>>,
    State(state): State<Arc<state::AppState>>,
) -> Html<String> {
    // already redirected back to homepage by the authority
    // with a code in query string parameters
    if let Some(auth_code) = auth_code {
        assert!(!auth_code.state.is_empty());
        // exchange the code they brought from the authority for an access token
        let access_token =
            utils::exchange_code_for_token(&state.oauth_client, auth_code.code.clone()).await;
        // instantiate an authenticator
        let outlook_oauth = OutlookOAuth2 {
            user: env::var("EMAIL_ADDRESS").unwrap(),
            access_token,
        };
        // establish a connection to the IMAP server
        let client = imap::ClientBuilder::new("outlook.office365.com", 993)
            .connect()
            .expect("successfly connected");
        // start a session
        let mut session = client
            .authenticate("XOAUTH2", &outlook_oauth)
            .expect("authenticated connection");
        // fetch the first email in the inbox
        session.select("INBOX").unwrap();
        let messages = session.fetch("1", "RFC822").unwrap();
        let first_message = messages.iter().next().unwrap().body().unwrap();
        let first_message = std::str::from_utf8(first_message).unwrap();
        // end the session
        session.logout().unwrap();
        // render the contents of the fetched email
        let page = format!(
            r#"
            <!doctype html>
                <html lang="en">
                <head>
                    <title>Home</title>  
                    <meta name="viewport" content="width=device-width"> 
                </head>
                <body>
                    <span>First message in your inbox</span>
                    <span>{:?}</span>
                </body>
            </html>
            "#,
            first_message,
        );
        return Html(page);
    }
    // should go and log in to their microsoft account (if not already logged in)
    // and explicitly authorize the OAuth application, after which the authority
    // will issue a code that can be later on echanged for an access token
    let url = utils::build_auth_url(&state.oauth_client);
    let page = format!(
        r#"
        <!doctype html>
            <html lang="en">
            <head>
                <title>Home</title>  
                <meta name="viewport" content="width=device-width"> 
            </head>
            <body>
                <a href="{}">Show my fist letter</a>
            </body>
        </html>
        "#,
        url.to_string()
    );
    Html(page)
}
