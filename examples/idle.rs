use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "idle")]
struct Opt {
    // The server name to connect to
    #[structopt(short, long)]
    server: String,

    // The port to use
    #[structopt(short, long, default_value = "993")]
    port: u16,

    // The account username
    #[structopt(short, long)]
    username: String,

    // The account password. In a production system passwords
    // would normally be in a config or fetched at runtime from
    // a password manager or user prompt and not passed on the
    // command line.
    #[structopt(short = "w", long)]
    password: String,

    // The mailbox to IDLE on
    #[structopt(short, long, default_value = "INBOX")]
    mailbox: String,

    #[structopt(
        short = "x",
        long,
        help = "The number of responses to receive before exiting",
        default_value = "5"
    )]
    max_responses: usize,
}

fn main() {
    let opt = Opt::from_args();

    let client = imap::ClientBuilder::new(opt.server.clone(), opt.port)
        .native_tls()
        .expect("Could not connect to imap server");

    let mut imap = client
        .login(opt.username, opt.password)
        .expect("Could not authenticate");

    // Turn on debug output so we can see the actual traffic coming
    // from the server and how it is handled in our callback.
    // This wouldn't be turned on in a production build, but is helpful
    // in examples and for debugging.
    imap.debug = true;

    imap.select(opt.mailbox).expect("Could not select mailbox");

    // Implement a trivial counter that causes the IDLE callback to end the IDLE
    // after a fixed number of responses.
    //
    // A threaded client could use channels or shared data to interact with the
    // rest of the program and update mailbox state, decide to exit the IDLE, etc.
    let mut num_responses = 0;
    let max_responses = opt.max_responses;
    let idle_result = imap.idle().wait_while(|response| {
        num_responses += 1;
        println!("IDLE response #{}: {:?}", num_responses, response);
        if num_responses >= max_responses {
            // Stop IDLE
            false
        } else {
            // Continue IDLE
            true
        }
    });

    match idle_result {
        Ok(reason) => println!("IDLE finished normally {:?}", reason),
        Err(e) => println!("IDLE finished with error {:?}", e),
    }

    imap.logout().expect("Could not log out");
}
