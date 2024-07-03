use std::process;
use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::net::TcpStream;

use clap::Parser;
use ctrlc;

use hello_http::ServerConfig;


const RESOURCE_DIR : &str = "../res";

/// Fixed configuration for the web app.
struct AppConfig<'a> {
    name: &'a str,
    version: &'a str,
}

/// Dynamic application state.
struct AppState {
    req_cnt: usize,
}


/// Entry point
fn main() {

    // Run the server and handle fatal errors
    if let Err(e) = run() {
        eprintln!("{:?}", e);
        process::exit(1);
    }
    else {
        process::exit(0);
    }
}


/// Load configuration and runs the server.
fn run() -> Result<(), Box<dyn Error>> {

    // Determine static configuration
    let res_dir = exe_relative_dir(Path::new(RESOURCE_DIR)).or_else(
        |e| Err( format!("Unable to locate application resource files: {:?}", e))
    )?;

    // Build dynamic config from command-line and default values:
    let mut config = ServerConfig::parse();

    // Merge static and dynamic configuration
    println!("Resource dir: {:?}", res_dir);
    config.resource_dir = res_dir;

    
    let app_config = AppConfig {
        name: "My Web App",
        version: "0.1",
    };

    let app_state = Arc::new(RwLock::new(AppState {
        req_cnt: 0,
    }));

    let enabled_til_ctrlc = set_ctrlc_finalizer(&config);

    // Run the server
    hello_http::run(enabled_til_ctrlc, config, move |request|{
        process_request(request, &app_config, Arc::clone(&app_state))
    })?;

    Ok(())
}


fn set_ctrlc_finalizer(config: &ServerConfig) -> Arc<AtomicBool> {

    // Will run the server until this value becomes `false`:
    let is_server_enabled = Arc::new( AtomicBool::new(true) );
    let enabled = Arc::clone(&is_server_enabled);

    let self_address = format!("{}:{}", config.interface_address, config.port);

    // Set handler for the TERM signal to shutdown the server:
    ctrlc::set_handler(move ||
    {
        println!(" TERM signal (Ctrl-C) received, will shut server down ...");

        // Flag the server as disabled:
        enabled.store(false, Ordering::Relaxed);

        // Create a dummy connection to the server to ensure the socket gets unblocked:
        let _ = TcpStream::connect(&self_address);
    }
    ).unwrap_or_else(|err| {
        eprintln!("WARN: Failed to set handler for TERM signal (Ctrl-C): {err}");
    });

    is_server_enabled
}


use hello_http::http;

/// This is the router
fn process_request(
    header: &http::Request, app_config: &AppConfig, app_state: Arc<RwLock<AppState>>)
    -> Result<http::Response, Box<dyn Error>>
{
    use hello_http::http:: {
        req::Method::*,
        Response,
        res::Status,
        res::Content::*,
    };

    use std::time::SystemTime;
    use std::time::Duration;
    use std::thread;

    // Update app state
    let mut req_cnt = app_state.read().unwrap().req_cnt;
    req_cnt += 1;
    println!("Request #{req_cnt}");
    app_state.write().unwrap().req_cnt = req_cnt;

    // Resolve route:
    let response = match &header.method {

        Get(uri) if uri == "/" => Response {
            status: Status::OK,
            content: ServerFile("hello.html".into()),
        },

        Get(uri) if uri == "/info" => Response {
            status: Status::OK,
            content: Text( format!("{}\nVersion: {}\nRequests: {req_cnt}", app_config.name, app_config.version) ),
        },

        Get(uri) if uri == "/time" => {
            let unix_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
            Response {
                status: Status::OK,
                content: Text( format!("Unix time: {}", unix_time.as_secs()) ),
            }
        },

        Get(uri) if uri == "/go" => Response {
            status: Status::OK,
            content: ServerFile("hello.html".into()),
        },

        Get(uri) if uri == "/sleep" => {
            thread::sleep(Duration::from_secs(5));
            Response {
                status: Status::OK,
                content: UserFile("hello.html".into()),
            }
        },

        _ => {
            Response {
                status: Status::NotFound,
                content: UnknownRoute,
            }
        },
    };

    Ok(response)
}


/// Locates a directory relative to the running executable and returns it as
/// an absolute, canonical path.
fn exe_relative_dir(rel_path: &Path) -> Result<PathBuf, Box<dyn Error>> {

    let mut exe_path = env::current_exe()?;     // current exe path

    exe_path.pop();                             // exe parent dir
    exe_path.push(rel_path);                    // relative path appended
    let abs_dir = exe_path.canonicalize()?;     // resolve path

    match abs_dir.is_dir() {
        true  => Ok(abs_dir),
        false => Err( format!("Not a directory: {:?}", abs_dir).into() ),
    }
}

