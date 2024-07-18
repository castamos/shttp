/// Man page reader and formatter to HTML.
/// External formatter tools are required.
///
/// Example:
/// ```
/// use man_reader::Reader;
/// match Reader::Man2Html.man_as_html("bash") {
///     ManPage(html) => println!(html),
///     NotFound(_)   => eprintln!("Man page not found"),
/// };
/// ```

use std::error::Error;
use std::process::Command;

use clap::ValueEnum;


pub enum ManOut {
    ManPage(String),
    NotFound(String),
}


pub struct ManReader<'a> {
    filter_cmd  : &'a str,
    filter_args : &'a [&'a str],
    man_args    : &'a [&'a str],
}


#[allow(dead_code)]
#[derive(Debug, Clone, ValueEnum)]
pub enum Reader {
    Man,
    Roffit,
    Man2Html,
    Pandoc,
}


pub const fn get_reader(reader_id: &Reader) -> ManReader<'static> {
    use Reader::*;

    match reader_id {
        Man     => ManReader {
            man_args    : &[ "--html=cat" ],
            filter_cmd  : "cat", // "identity"
            filter_args : &[],
        },
        Roffit  => ManReader {
            man_args    : &["-R", "UTF-8"],
            filter_cmd  : "roffit",
            filter_args : &[],
        },
        Man2Html => ManReader {
            man_args    : &["-R", "UTF-8"],
            filter_cmd  : "man2html",
            filter_args : &["-r"],
        },
        Pandoc  => ManReader {
            man_args    : &["-R", "UTF-8"],
            filter_cmd  : "pandoc",
            filter_args : &["-r", "man", "-t", "html5"],
        }
    }
}


impl Reader {
    pub fn man_as_html(&self, page_name: &str) -> Result<ManOut, Box<dyn Error>> {
        get_reader(self).man_as_html(page_name)
    }
}

impl<'a> ManReader<'a> {

    /// Returns a man page formated as HTML
    pub fn man_as_html(&self, page_name: &str) -> Result<ManOut, Box<dyn Error>> {

        use std::process::Stdio;

        let mut man_cmd = Command::new("man")
            .args(self.man_args)
            .arg(page_name)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let man_out  = man_cmd.stdout.take().ok_or("Cannot read stdout from `man`")?;

        let filter_cmd = Command::new(self.filter_cmd)
            .args(self.filter_args)
            .stdin(Stdio::from(man_out))
            .stdout(Stdio::piped())
            .spawn()?;

        // TODO: Set a timeout here:
        let cmd_result = filter_cmd.wait_with_output()?;
        let mut out_str = String::from_utf8(cmd_result.stdout)?;

        if self.filter_cmd == "man2html" {
            // Discard HTML headers
            let mut lines = out_str.lines();
            while let Some(line) = lines.next() {
                if line == "" { break; }
            }
            out_str = lines.collect();
        }

        let man_result = man_cmd.wait_with_output()?;

        if man_result.status.success() {
            Ok( ManOut::ManPage(out_str) )
        }
        else {
            let man_err_str = String::from_utf8(man_result.stderr)?;
            out_str.push_str(&man_err_str);
            Ok( ManOut::NotFound(out_str) )
        }
    }
}

