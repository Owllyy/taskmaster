use std::env;
use std::process;
use std::path::{PathBuf};

pub fn get_config_from_args() -> PathBuf {
    let args: Vec<String> = env::args().collect();
    let mut path: PathBuf = PathBuf::new();
    match args.len() {
        2 => path.push(args[1].to_owned()),
        3.. => { eprintln!("Taskmaster: Too many arguments"); process::exit(1);},
        _ => { eprintln!("Taskmaster: Missing config file name"); process::exit(1);},
    };
    
    match path.try_exists() {
        Ok(true) => {},
        _ => { eprintln!("Taskmaster: Config file : Does not exist"); process::exit(1);}
    }
    if !path.is_file() {
        eprintln!("Taskmaster: Config file : Is not a file"); 
        process::exit(1);
    }
    match path.extension() {
        Some(x) => { if x != "conf" {
            eprintln!("Taskmaster: Config file : Bad extention"); process::exit(1);
        }},
        None => { eprintln!("Taskmaster: Config file : Bad extention"); process::exit(1);}
    }

    path
} 