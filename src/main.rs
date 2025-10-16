use clap::Parser;
use fs2::FileExt;
use log::{error, info, warn, debug};
use regex::Regex;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const RET_CANNOT_LOCK: i32 = 1;
const RET_IS_DIR: i32 = 2;
const WAIT_TIME: u64 = 10; // interval for file check in seconds
const RET_FILE_MISSING: i32 = 3;


#[derive(Parser)]
#[command(version, about, long_about=None)]
struct Args {
    #[arg(short, long)]
    filename: String,

    /// Use if needed to wait for file to be updated
    #[arg(short, long)]
    update: bool,
}

fn main() -> Result<(), i32> {
    let args = Args::parse();

    let filename = args.filename;

    env_logger::init();

    // Create and lock the stale file

    let (lock, lock_file) = create_lock_file(&filename);
    match lock.try_lock_exclusive() {
        Ok(()) => {
            info!("Stale file generated '{}'", &lock_file);

            if args.update {
                match wait_for_file_update(&filename) {
                    Ok(()) => {
                        remove_lock_file(&lock_file);
                        return Ok(());
                    }

                    Err(ret) => return Err(ret),
                }
            } else {
                wait_for_file(&filename);
            }
        }
        Err(e) => {
            error!("Cannot obtain lock on '{}': {}", &lock_file, e);
            return Err(RET_CANNOT_LOCK);
        }
    }

    remove_lock_file(&lock_file);
    Ok(())
}

fn wait_for_file_update(filename: &String) -> Result<(), i32> {
    if fs::exists(&filename).unwrap() {
        let last_mod = get_last_mod(&filename).unwrap();

        //let mut latest_mod: u64;

        loop {
            let last_mod_res = get_last_mod(filename);
            match last_mod_res {
                Ok(latest_mod) => {
                    if last_mod < latest_mod {
                        info!("File updated, exiting...");
                        return Ok(());
                    }
                    sleep(Duration::from_secs(WAIT_TIME));
                }
                Err(ret) => return Err(ret),
            }
        }
    } else {
        warn!("File '{}' does not exist. Waiting...", &filename);
        wait_for_file(filename);
        Ok(())
    }
}
//}

fn wait_for_file(filepath: &String) {
    let mut temp_filepath = filepath.clone();
    loop {
        if filepath.contains('*') {
            if let Some(filename) = resolve_file_name(&filepath) {
                temp_filepath = filename;
            }
        }
        if fs::exists(&temp_filepath).unwrap() {
            info!("File '{}' is available, bye...", &temp_filepath);
            return;
        }

        sleep(Duration::from_secs(WAIT_TIME));
    }
}

fn get_last_mod(file: &String) -> Result<u64, i32> {
    let metadata_res = fs::metadata(file);
    match metadata_res {
        Ok(metadata) => {
            if !metadata.is_dir() {
                let time = metadata.modified().unwrap();
                let last_mod = get_seconds(time);
                debug!("Duration till last mod: {}", last_mod);
                Ok(last_mod)
            } else {
                warn!(
                    "Cannot check file presence, '{}' is a directory. Exiting (retcode={})",
                    file, RET_IS_DIR
                );
                Err(RET_IS_DIR)
            }
        } 
        Err(_) => {error!("File '{}' went missing :(, restart again if you want to wait for it's arrival", &file);
                    return Err(RET_FILE_MISSING);
                    }
    }
}

// get filename incase of wildcards

fn resolve_file_name(filename: &String) -> Option<String> {
    let a = filename.rfind("/").unwrap();
    let (path, file) = filename.split_at(a + 1);

    let (part_a, part_b) = file.split_at(file.find("*").unwrap());
    let file_len = file.len()-1;
    let part_a_end = part_a.len()-1;
    debug!("Search file len {}, part a end {}", file_len, part_a_end);
    //let part_b_start: usize = part_a_end + 1;
    //let part_b_start = part_b.len()

    for item in fs::read_dir(path).unwrap() {
        let item = item.unwrap();
        let buf = item.path();
        let name = buf.file_name().unwrap().to_str().unwrap().to_string();


        if name.len() >= file_len{
        debug!("Starts with {}, ends with {}, full {}", 
            &name[0..part_a_end], &name[(name.len()-part_b.len())..], &name);
        // find the file if exist
        if name.starts_with(&part_a[0..part_a_end]) && name.ends_with(&part_b[1..]) {
            let abs_file_path = path.to_owned() + &name;
            return Some(abs_file_path);
        }
        }
    }

    //println!("{}, {}", path, file);
    None
    // file
}

fn get_seconds(modified: SystemTime) -> u64 {
    if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
        return duration.as_secs();
    } else {
        println!("Error getting duration  since EPOCH");
    }
    1
}

fn create_lock_file(filename: &String) -> (File, String) {
    let lock_name = sanitize(filename);
    let mut lock_path = env::var("HOME").unwrap() + "/filewatcher/";
    if !fs::exists(&lock_path).unwrap() {
        fs::create_dir(&lock_path).expect(&format!("Failed to create lock dir '{}'", lock_path));
    }
    lock_path.push_str(&lock_name);

    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&lock_path)
        .expect(&format!("Failed to open lock '{}'", &lock_path));

    (lock, lock_path)
}

fn sanitize(input: &String) -> String {
    let regex = Regex::new(r"[^a-zA-Z0-9]").unwrap();
    regex.replace_all(input, "_").to_string()
}

fn remove_lock_file(lock_file: &String) {
    fs::remove_file(lock_file).unwrap();
}
