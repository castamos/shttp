/// Web app example that allows browsing man pages.

// Standard modules
use std::error::Error;
use std::process;
use std::env;

// crates.io modules
use env_logger;
use log::{error, debug};
use clap::{Parser, Args, CommandFactory, FromArgMatches as _};

// The module this example is for
use shttp;
use shttp::{ServerConfig, Request, Response, Status, Content, Method};

// Modules specific to this example
mod man_reader;
use man_reader::*;

/// Default log level if not given in the environment
const DEFAULT_LOG_LEVEL : &str = "info";

/// A man page web browser
#[derive(Parser, Debug)]
struct AppConfig {

    /// Formatter for rendering man pages as HTML
    #[arg(value_enum, default_value_t=man_reader::Reader::Man, short, long)]
    formatter: man_reader::Reader,
}


/// Entry point.
fn main() {

    // Configure logging
    if let Ok(_) = env::var("RUST_LOG") {} else {
        // Set default log level if not given in the environment.
        env::set_var("RUST_LOG", DEFAULT_LOG_LEVEL);
    }
    env_logger::init();

    if let Err(e) = run() {
        error!("{:?}", e);
        process::exit(1);
    }
    else {
        process::exit(0);
    }
}


/// Runs the HTTP server
fn run() -> Result<(), Box<dyn Error>> {

    // Augment command-line arguments with those of the HTTP server:
    let cli = ServerConfig::augment_args(AppConfig::command());
    // Parse command line:
    let matches = cli.get_matches();

    // Get each config struct:
    let srv_config = ServerConfig::from_arg_matches(&matches)?;
    let app_config = AppConfig::from_arg_matches(&matches)?;
     
    debug!("App: {:?}", app_config);
    debug!("Srv: {:?}", srv_config);

    let enabled = shttp::set_ctrlc_finalizer(&srv_config);
    shttp::run(enabled, srv_config, router)?;

    Ok(())
}


/// Processes HTTP requests. Errors reading man pages are translated to HTML "Internal Server
/// Error" responses.
fn router(req: &Request) -> Result<Response, Box<dyn Error>> {
    match route_manpage(req) {

        Ok(resp) => Ok(resp),

        Err(e) => {
            let msg = format!("Failed to execute `man` command: {:?}", e);
            error!("{}", msg);

            Ok(Response {
                status:  Status::InternalError,
                content: Content::Text(msg),
            })
        }
    }
}


fn route_manpage(req: &Request) -> Result<Response, Box<dyn Error>> {

    let Method::Get(ref uri) = req.method else {
        return Ok(Response {
            status: Status::BadRequest,
            content: Content::Text("Only the GET method is supported".into()),
        });
    };

    if uri.len() < 1 {
        return Ok(Response {
            status: Status::BadRequest,
            content: Content::Text("Input too short".into()),
        });
    }

    let page_name = &uri[1..];

    let man_html = match Reader::Pandoc.man_as_html(page_name)? {
        ManOut::ManPage(html) => html,
        ManOut::NotFound(msg) => format!("<div class='error'>{}</div>", msg),
    };

    Ok(Response {
        status:  Status::OK,
        content: Content::Text(man_html),
    })
}

