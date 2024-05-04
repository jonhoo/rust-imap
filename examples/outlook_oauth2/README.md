# Prerequisites

To run this example, you will need a registered single-tenant OAuth application at `Azure`. Follow [this guide](https://learn.microsoft.com/en-us/exchange/client-developer/legacy-protocols/how-to-authenticate-an-imap-pop-smtp-application-by-using-oauth) if you do not have one just yet. Note that the example was created against a single-tenant OAuth app (i.e. for email addresses belonging to your organisation), so make sure to select an appropriate option when registering your application. Alternatively, you can create an app for personal microsoft accounts, but then make sure to specify `AZURE_OAUTH_APP_TENANT_ID=consumers` and `EMAIL_ADDRESS=<your-personal-not-corporate@email.address>` in your `.env` file (read further).

Important! When registering your app (or in the app's settings afterwards), specify `https://localhost:3993` as a redirect url and API permissions for the app should be at least `IMAP.AccessAsUser.All`. Also make sure to select `ID tokens (used for implicit and hybrid flows)` in the app's authentication settings.

Create a `.env` file in this example's directory filling in the required data (`.env.sample` has got an exhaustive list of variables needed for this example to work).

You can now hit `cargo run --example outlook_oauth2` from the repo's root and visit `https://localhost:3993` in your browser. You will want to tell your browser to _dangerously_ trust the self-signed certificates for this domain, but this is _only_ for testing / demo purposes.
