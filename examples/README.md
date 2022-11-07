Examples
========

This directory contains examples of working with the IMAP client.

Examples:
  * basic - This is a very basic example of using the client.
  * gmail_oauth2 - This is an example using oauth2 for logging into gmail as a secure appplication.
  * idle - This is an example showing how to use IDLE to monitor a mailbox.
  * rustls - This demonstrates how to use Rustls instead of Openssl for secure connections (helpful for cross compilation).
  * starttls - This is an example showing how to use STARTTLS after connecting over plaintext.
  * timeout - This demonstrates how to use timeouts while connecting to an IMAP server by using a custom TCP/TLS stream initialization and creating a `Client` directly instead of using the `ClientBuilder`.
  * plaintext - This demonstrates how to make an unencrypted IMAP connection (usually over 143) with a `Client` using a naked TCP connection.
