
pub type Request = req::Request;
pub type Response = res::Response;

/// HTTP Request
pub mod req {

    use std::error::Error;
    use std::net::TcpStream;
    use std::io::prelude::*;
    use std::collections::HashMap;
    use crate::uri;

    const HTTP_HEADER_MAX_LEN : usize = 1024 * 1;

    /// HTTP Request Methods
    #[derive(Debug)]
    pub enum Method {
        Get(String),
        Put(String),
    }


    /// HTTP Request Header
    #[derive(Debug)]
    pub struct Request {
        pub method:     Method,
        pub headers:    HashMap::<String, String>,
        pub warnings:   Vec::<String>,
    }


    impl Request {

        /// Parses an HTTP header given as a raw string and returns the corresponding
        /// `Request` object.
        pub fn parse(header: &str) -> Result<Request, Box<dyn Error>> {

            let mut warnings: Vec<String> = vec![];
            let mut lines = header.lines();

            // First line in the header is the URI request.

            let method = if let Some(request) = lines.next() {
                // First line has the URI request
                let fields: Vec<_> = request.split_ascii_whitespace().collect();

                let [method_field, raw_uri, http_version] = fields[..] else {
                    return Err("Missing fields in URI in header.")?;
                };

                if http_version != "HTTP/1.1" {
                    warnings.push(format!("Unknown HTTP version {}", http_version));
                }

                let Ok(uri) = uri::decode_uri(raw_uri) else {
                    return Err("Encoded URL does not represent valid UTF-8: {raw_uri}")?;
                };

                match method_field.to_ascii_uppercase().as_str() {
                    "GET" => Method::Get(uri),
                    "PUT" => Method::Put(uri),
                    _ => return Err(
                        format!("Unknown HTTP method: {}", method_field).into()
                    ),
                }
            }
            else {
                return Err("Could not find URI in header.".into());
            };

            // Remaining lines in the header are HTTP header fields.
           
            let mut headers = HashMap::<String, String>::new();

            for line in lines {
                let colon_pair: Vec<_> = line.splitn(2, ':').collect();
                
                if let [name, value] = colon_pair[..] {
                    headers.insert(name.trim().into(), value.trim().into());
                }
                else {
                    warnings.push(format!(
                        "Invalid header line, missing colon separator in: '{line}'"
                    ));
                }
            }

            Ok(Request { method, headers, warnings })
        }


        pub fn parse_from_stream(stream: &mut TcpStream) ->
            Result<Request, Box<dyn Error>>
        {
            let request_header = retrieve_header(stream)?;
            Request::parse(&request_header[..])
        }

    } // impl Request


    fn retrieve_header(stream: &mut TcpStream) -> Result<String, Box<dyn Error>> {
        // Look at most the first 1KB
        let mut buf = [0; HTTP_HEADER_MAX_LEN];
        let _len = stream.peek(&mut buf)?;

        let buf_str = String::from_utf8_lossy(&buf);

        for terminator in [ "\r\n\r\n", "\n\n" ] {

            if let Some(end_index) = buf_str.find(terminator) {

                // Get the header
                let mut head_buf = Vec::with_capacity(end_index);
                head_buf.resize(end_index, 0);
                stream.read_exact(&mut head_buf[..])?;

                // Discard separator
                let mut _sep_buf = Vec::with_capacity(terminator.len());
                _sep_buf.resize(terminator.len(), 0);
                stream.read_exact(&mut _sep_buf)?;

                return Ok(String::from_utf8_lossy(&head_buf).to_string());
            }
        }

        // No terminator matched:
        Err( format!(
            "Could not find header terminator in the first {HTTP_HEADER_MAX_LEN} \
             bytes. Header: {buf_str}"
        ).into())
    }

} // mod Request


/// HTTP Response
pub mod res {

    use std::path::PathBuf;
    use std::fs;
    use log::error;

    /// HTTP Response Status
    pub enum Status {
        OK,
        BadRequest,
        NotFound,
        InternalError,
    }

    impl Status {

        pub fn as_str(&self) -> &'static str {
            use Status::*;
            match self {
                OK              => "HTTP/1.1 200 OK",
                BadRequest      => "HTTP/1.1 400 BAD REQUEST",
                NotFound        => "HTTP/1.1 404 NOT FOUND",
                InternalError   => "HTTP/1.1 500 INTERNAL SERVER ERROR",
            }
        }
    }


    /// The actual HTTP response data to send
    pub struct TextResponse {
        pub status: Status,
        pub body: String,
    }

    impl TextResponse {
        pub fn as_string(&self) -> String {
            // FIXME: Avoid copying `body`, perhaps by returning a string iterator.
            let status_str = self.status.as_str();
            let mut response = format!("{}\r\nContent-Length: {}\r\nCache-Control: no-store, no-cache, must-revalidate\r\n\r\n", status_str, self.body.len());
            response.push_str(&self.body);
            response
        }
    }


    /// HTTP Response Content
    pub enum Content {
        ServerFile(PathBuf),
        UserFile(PathBuf),
        Text(String),
        UnknownRoute,
        // TODO: Maybe add `Stream`?
    }

    /// HTTP response get from routers
    pub struct Response {
        pub status: Status,
        pub content: Content,
    }


    impl Response {

        pub fn into_text_response(self, server_path: &PathBuf) -> TextResponse {

            use Content::*;

            let mut response = self;

            loop {
                // Transform `response` until we get `Text`
                response = match response.content {

                    Text(text) => return TextResponse {
                        status: response.status,
                        body: text,
                    },

                    UserFile(abs_path) => {
                        match fs::read_to_string(&abs_path)
                        {
                            Ok(file_text) => Response {
                                status:  response.status,
                                content: Text(file_text),
                            },
                            Err(e) => {
                                error!("Failed to read '{:?}': {:?}", abs_path, e);
                                Response {
                                    status: Status::InternalError,
                                    content: Text("Resource not available.".into()),
                                }
                            },
                        }
                    },

                    ServerFile(rel_path) => {
                        let mut abs_path = server_path.clone();
                        abs_path.push(rel_path);
                        Response {
                            status: response.status,
                            content: UserFile(abs_path),
                        }
                    },

                    UnknownRoute => Response {
                        status: Status::NotFound,
                        content: ServerFile("404.html".into()),
                    },
                };
            } // loop
            // The compiler knows this point is `unreachable!()`.
        } // fn

    } // impl
}

