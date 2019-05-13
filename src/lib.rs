use std::env;
use std::error::Error;

use std::fs;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use std::sync::mpsc;
use std::thread;

pub struct Config {
    verbose: bool,
    filename: Option<String>,
    ignore_filename_case: bool,
    content: Option<String>,
    ignore_content_case: bool,
    dop: usize,
    root: PathBuf,
}

impl Config {
    pub fn new(matches: &clap::ArgMatches) -> Result<Config, Box<dyn Error>> {
        let verbose = matches.is_present("verbose");

        let filename = match matches.value_of("filename") {
            Some(s) => Some(String::from(s)),
            None => None,
        };

        let ignore_filename_case = matches.is_present("ignore-filename-case");

        let content = match matches.value_of("content") {
            Some(s) => Some(String::from(s)),
            None => None,
        };

        let ignore_content_case = matches.is_present("ignore-content-case");

        let dop = match matches.value_of("dop") {
            Some(s) => String::from(s),
            None => num_cpus::get().to_string(),
        };

        let dop = match dop.parse::<usize>() {
            Ok(dop) => dop,
            Err(_) => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid degree of parallelism",
                )));
            }
        };

        let root = match matches.value_of("root") {
            Some(s) => {
                let path = PathBuf::from(s);
                if !path.is_dir() {
                    return Err(Box::new(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "working directory is not a directory",
                    )));
                }

                path
            }
            None => env::current_dir()?,
        };

        Ok(Config {
            verbose,
            filename,
            ignore_filename_case,
            content,
            ignore_content_case,
            dop,
            root,
        })
    }
}

struct LpsResult {
    file: String,
    lines: Option<Vec<LpsLineResult>>,
}

struct LpsLineResult {
    line: u32,
    column: u32,
    content: String,
}

pub fn run(config: Config) -> Result<(), Box<dyn Error>> {
    if config.verbose {
        let root_path = config.root.to_str();
        if root_path.is_none() {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid working directory",
            )));
        }

        println!("working directory: {}", root_path.unwrap());
        println!("DoP was set to {} threads", config.dop);
    }

    // Get all files that match name, size, attributes, ...
    let files = find_files_by_name(&config, &config.root);

    // Check content in multiple threads
    let (sender, receiver) = mpsc::channel::<LpsResult>();

    content_search(&config, files, sender);

    // Aggregate results
    loop {
        let result = match receiver.recv() {
            Ok(res) => res,
            Err(_) => {
                // This will occur when all threads have finished
                break;
            }
        };

        if result.lines.is_none() {
            // lines is none if no content search was performed, just print the file names
            println!("{}", result.file);
        } else {
            let lines = result.lines.unwrap();
            if !lines.is_empty() {
                println!("{}", result.file);
                for line in lines {
                    println!("  {}:{} {}", line.line, line.column, line.content);
                }
            }
        }
    }

    Ok(())
}

fn find_files_by_name(config: &Config, path: &PathBuf) -> Vec<PathBuf> {
    let mut result = Vec::new();

    result
}

fn content_search(config: &Config, files: Vec<PathBuf>, sender: mpsc::Sender<LpsResult>) {
    if config.content.is_none() {
        // Just yield found files if content search is not requested
        for file in files {
            let file = file.to_string_lossy().to_string();
            if sender
                .send(LpsResult {
                    file: file,
                    lines: None,
                })
                .is_err()
            {
                break;
            }
        }
    } else {
        for chunk in files.chunks(files.len() / config.dop) {
            let chunk = chunk.to_vec();

            thread::spawn(move || {
                for file in chunk {
                    let file = match File::open(file) {
                        Ok(f) => f,
                        Err(_) => {
                            continue;
                        }
                    };

                    for line in BufReader::new(file).lines() {
                        let line = match line {
                            Ok(l) => l,
                            Err(_) => {
                                continue;
                            }
                        };
                    }
                }
            });
        }
    }
}
