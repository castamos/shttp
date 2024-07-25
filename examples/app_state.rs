use std::process;
use std::env;
use std::error::Error;
use std::path::PathBuf;
use std::path::Path;
use std::sync::{Arc, RwLock};

// For parsing command line:
use clap::{Command, Args, FromArgMatches as _};
// Logging:
use log::{error, info};
use env_logger;

// Our HTTP server:
use shttp::{ServerConfig, http};

/// Path for server html files, relative to the executable
const RESOURCE_DIR : &str = "../../res";
/// Default log level if not given in the environment
const DEFAULT_LOG_LEVEL : &str = "info";


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

    // Configure logging
    if let Ok(_) = env::var("RUST_LOG") {} else {
        // Set default log level if not given in the environment.
        env::set_var("RUST_LOG", DEFAULT_LOG_LEVEL);
    }
    env_logger::init();

    // Run the server and handle fatal errors
    if let Err(e) = run() {
        error!("{:?}", e);
        process::exit(1);
    }
    else {
        process::exit(0);
    }
}


/// Loads configuration and runs the server.
fn run() -> Result<(), Box<dyn Error>> {

    // Determine static configuration:
    let res_dir = exe_relative_dir(Path::new(RESOURCE_DIR)).or_else(
        |e| Err( format!("Unable to locate application resource files: {:?}", e))
    )?;

    // Build dynamic config from command-line and default values:
    let app_command = ServerConfig::augment_args( Command::new("Example App") );
    let mut config  = ServerConfig::from_arg_matches( &app_command.get_matches() )?;

    // Merge static and dynamic configuration:
    info!("Resource dir: {:?}", res_dir);
    config.resource_dir = res_dir;
    
    // Initialize application-specific fixed configuration:
    let app_config = AppConfig {
        name: "My Web App",
        version: "0.1",
    };

    // Initialize application-specific shared state:
    let app_state = Arc::new(RwLock::new(AppState {
        req_cnt: 0,
    }));

    // Configure server finalization via Ctrl-C
    let enabled_til_ctrlc = shttp::set_ctrlc_finalizer(&config);

    // Run the server
    shttp::run(enabled_til_ctrlc, config, move |request|{
        process_request(request, &app_config, Arc::clone(&app_state))
    })?;

    Ok(())
}


/// The HTTP endpoint router. This is called from each request the server receives
/// and may be called from different threads each time.
fn process_request(
    header: &http::Request, app_config: &AppConfig, app_state: Arc<RwLock<AppState>>)
    -> Result<http::Response, Box<dyn Error>>
{
    use shttp::http:: {
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
    info!("Request #{req_cnt}");
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
                content: ServerFile("hello.html".into()),
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
        false => Err( format!("Not a directory: {:?}", abs_dir) )?,
    }
}

