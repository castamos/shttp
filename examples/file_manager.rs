use std::process;
use std::env;
use std::error::Error;
use std::path::{Path, PathBuf, Component};
use std::sync::{Arc, RwLock};

// For parsing command line:
use clap::{Parser, Args, CommandFactory, FromArgMatches as _};
// Logging:
use log::{error, info, debug};
use env_logger;

// HTML generation
use maud::{html, Markup, PreEscaped};

// Our HTTP server:
use shttp::{ServerConfig, http};

/// Default log level if not given in the environment
const DEFAULT_LOG_LEVEL : &str = "info";


/// Fixed configuration for the web app.
#[derive(Debug, Default)]
struct AppInfo<'a> {
    name: &'a str,
    version: &'a str,
}

// Dynamic configuration from the command line.
#[derive(Parser, Debug)]
struct AppConfig<'a> {
    #[arg(short, long)]
    root_dir: PathBuf,

    #[arg(skip)]
    app_info: AppInfo<'a>,
}

/// Dynamic application state.
struct AppState {
    req_cnt: usize,
}


/// Entry point
fn main() {

    // Configure logging
    if env::var("RUST_LOG").is_err() {
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

    // Augment command-line arguments with those of the HTTP server:
    let cli = ServerConfig::augment_args(AppConfig::command());
    // Parse command line:
    let matches = cli.get_matches();

    // Get each config struct:
    let srv_config = ServerConfig::from_arg_matches(&matches)?;
    let mut app_config = AppConfig::from_arg_matches(&matches)?;

    // Initialize application-specific fixed configuration:
    app_config.app_info = AppInfo {
        name: "Web File Manager",
        version: "0.1",
    };

    // Initialize application-specific shared state:
    let app_state = Arc::new(RwLock::new(AppState {
        req_cnt: 0,
    }));

    // Configure server finalization via Ctrl-C
    let enabled_til_ctrlc = shttp::set_ctrlc_finalizer(&srv_config);

    // Run the server
    shttp::run(enabled_til_ctrlc, srv_config, move |request|{
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

    // Update app state
    let mut req_cnt = app_state.read().unwrap().req_cnt;
    req_cnt += 1;
    info!("Request #{req_cnt}");
    app_state.write().unwrap().req_cnt = req_cnt;

    // Resolve route:
    let response = match &header.method {

        Get(uri) if uri == "/info" => Response {
            status: Status::OK,
            content: Text( format!("{}\nVersion: {}\nRequests: {req_cnt}",
                app_config.app_info.name, app_config.app_info.version
            )),
        },

        Get(uri) => {

            if let Ok(rel_path) = sanitized_path_components(Path::new(uri)) {

                let mut abs_path = app_config.root_dir.clone();
                abs_path.push(&rel_path);

                if abs_path.is_dir() {
                    Response {
                        status: Status::OK,
                        content: Text( render_dir(&abs_path, &rel_path).into() ),
                    }
                }
                else if abs_path.is_file() {
                    Response {
                        status: Status::OK,
                        content: Text(format!("File OK: {:?}", abs_path)),
                    }
                }
                else {
                    Response {
                        status: Status::NotFound,
                        content: Text(format!("Path not found on server: {:?}", abs_path)),
                    }
                }
            }
            else {
                Response {
                    status: Status::BadRequest,
                    content: Text( "Invalid path".into() )
                }
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


/// Returns an iterator to the "sanitized" components of a path.
/// Returns error if the path contains `.` or `..`.
fn sanitized_path_components(path: &Path) -> Result<PathBuf, Box<dyn Error>> {

    let mut sanitized = PathBuf::new();

    for component in path.components() {
        match component {
            // Append "normal" components
            //Component::Normal(comp_str) => sanitized.push(comp_str),
            Component::Normal(comp_str) => {
                debug!("{:?}", comp_str);
                sanitized.push(comp_str)
            },

            // Discard root
            Component::RootDir => {},

            // Any other component is considered invalid
            _ => return Err("Invalid component")?,
        }
    }

    return Ok(sanitized);
}


/// Returns an HTML page (as string) listing the contents of the local directory
/// `dir_path`.
fn render_dir(full_path: &Path, suffix: &Path) -> Markup {

    debug!("Rendering directory: {:?}", suffix);
    let children = full_path.read_dir().expect("Cannot list directory contents");

    let space = html! { span { ( PreEscaped("&nbsp;") ) } };

    html! {
        p { "Contents of: " (space) (space)
            @let mut cur_path = PathBuf::from("/");
            button { a href="/" { "/" } }
            @for comp in suffix.components() {
                @let comp_str = comp.as_os_str().to_str().unwrap();
                @let _ = cur_path.push(comp);
                @let cur_path_str = cur_path.to_str().unwrap();
                button { a href=(cur_path_str) { (comp_str) "/" } }
            }
        }
        ul {
            @for entry in children {
                @match entry {
                    Ok(entry) => {

                        // The basename:
                        @let file_name = entry.file_name();
                        @let file_name_str = file_name.to_string_lossy();

                        // Terminator caracter to identify the "type" of the entry:
                        @let terminator = if entry.path().is_dir() { "/" } else { "" };

                        // Hyperlink
                        // TODO: Could we just use a relative path for the link?
                        @let mut path_hlink = PathBuf::from("/");
                        @let _ = path_hlink.push(suffix);
                        @let _ = path_hlink.push(&file_name);
                        @let path_hlink_str = path_hlink.to_string_lossy();

                        li { a href=(path_hlink_str) { (file_name_str) (terminator) } }
                    },
                    Err(_) => {
                        li { "<Invalid directory entry>" }
                    }
                }
            }
        }
    }
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

