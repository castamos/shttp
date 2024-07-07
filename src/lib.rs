use std::{
    error::Error,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    io::prelude::*,
};

use clap::Parser;
use ctrlc;

mod thread_pool;
use crate::thread_pool::ThreadPool;

pub mod http; // (`pub` required to re-export the module to main.rs)

// `ServerConfig` is the application configuration definition with embeded
// command-line parsing annotations. Doc-comments here are help strings.
//
/// A simple HTTP server
#[derive(Parser, Debug)]
pub struct ServerConfig {
    /// TCP port in which the server will listen to HTTP requests
    #[arg(short, long, default_value_t=7878)]
    pub port: u32,

    /// IP address or hostname to identify the network interfaces in which to
    /// listen to requests (`0.0.0.0` means lesten in all interfaces)
    #[arg( short, long, default_value_t={"0.0.0.0".to_string()} )]
    pub interface_address: String,

    /// Number of worker threads
    #[arg(short, long, default_value_t=8)]
    pub threads: usize,

    #[arg(skip)]
    pub resource_dir: PathBuf,
}


/// Executes the HTTP server and keeps it running until the shared boolean flag `enabled` is changed (externally
/// from other thread) to `false`, at which point the next connection attempt makes this function return.
/// (Therefore a dummy connection is required to signal the server finalization.)
///
/// All requests are processed by the given `router` closure. The parsed `Request` is passed to it,
/// and the `Response` it returns is used as the server response for that specific request.
///
/// All network and runtime configuration is passed in `config`.
///
pub fn run<F>(enabled: Arc<AtomicBool>, config: ServerConfig, router: F) -> Result<(), Box<dyn Error>>
where
    F: Fn(&http::Request) -> Result<http::Response, Box<dyn Error>> + Send + 'static + Sync
{

    let bind_address = format!("{}:{}", config.interface_address, config.port);
    println!("Binding server to {bind_address}");

    let listener = TcpListener::bind(bind_address)?;
    let pool = ThreadPool::new(config.threads);
    let shared_config = Arc::new(config);
    let shared_router = Arc::new(router);

    for stream_result in listener.incoming() {
        if !enabled.load(Ordering::Acquire) {
            break;
        }
        let stream = stream_result.or_else(|e| Err(e))?; // graceful unwrap().
        let shared_config = Arc::clone(&shared_config);
        let shared_router = Arc::clone(&shared_router);
        pool.execute(move || {
            handle_connection(stream, shared_config, shared_router);
        });
    }

    println!("Server closed, not more connections will be accepted.");

    Ok(()) // Everything was OK.
}


/// Processes the connection given in `stream` by reading and parsing it as an HTTP `Request` that
/// then is passed to the user-provided HTTP `router` closure, which is expected to return a
/// structured HTTP `Response` that finally is serialized and written back to `stream`.
///
fn handle_connection<F>(mut stream: TcpStream, config: Arc<ServerConfig>, router: Arc<F>)
where
    F: Fn(&http::Request) -> Result<http::Response, Box<dyn Error>> + Send + 'static + Sync
{
    let text_response = match http::Request::parse_from_stream(&mut stream)
    {
        Ok(request) => {
            println!("Request header: {:?}", request);
            match router(&request)
            {
                Ok(response) => response.into_text_response(&config.resource_dir),
                Err(error) => {
                    println!("Router failed to process request: {error}");
                    http::res::TextResponse {
                        status: http::res::Status::InternalError,
                        body: "Failed to process resquest".into(),
                    }
                }
            }
        },
        Err(error) => {
            println!("Bad request: {error}");
            http::res::TextResponse {
                status: http::res::Status::BadRequest,
                body: "Bad request".into(),
            }
        },
    };

    send_response(&mut stream, &text_response);
}


/// Serializes the given `response` and writes it to `stream`.
fn send_response(stream: &mut TcpStream, response: &http::res::TextResponse) {

    let raw_response = response.as_string();

    println!("Response: {:#?}", raw_response);
    stream.write_all(raw_response.as_bytes()).unwrap_or_else(|error| {
        println!("ERROR: Failed to write response: {:?}", error);
    });
}


/// Helper function to set a handler for the TERM signal or equivalent
/// (Ctrl-C). Returns a thread-safe boolean flag that changes its value
/// to `false` when the signal is received; this flag can be passed as
/// the `enabled` parameter for `run(...)` so that the server terminates
/// gracefully.
///
pub fn set_ctrlc_finalizer(config: &ServerConfig) -> Arc<AtomicBool> {

    // Will run the server until this value becomes `false`:
    let is_server_enabled = Arc::new( AtomicBool::new(true) );
    let enabled = Arc::clone(&is_server_enabled);

    let self_address = format!("{}:{}", config.interface_address, config.port);

    // Set handler for the TERM signal to shutdown the server:
    ctrlc::set_handler(move ||
    {
        println!(" TERM signal (Ctrl-C) received, will shut server down ...");

        // Flag the server as disabled:
        enabled.store(false, Ordering::Release);

        // Create a dummy connection to the server to ensure the socket gets unblocked:
        let _ = TcpStream::connect(&self_address);
    }
    ).unwrap_or_else(|err| {
        eprintln!("WARN: Failed to set handler for TERM signal (Ctrl-C): {err}");
    });

    is_server_enabled
}
