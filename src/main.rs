use chrono::Local;
use clap::{App, Arg};
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{self, File};
use std::io; //{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::exit;
use zip::write::{FileOptions, ZipWriter};
use zip::ZipArchive;

#[allow(unused_must_use)]
fn main() {
    // Define CLI using clap
    let matches = App::new("Time Capsule CLI")
        .version("1.0")
        .author("Gh. Ibr.")
        .about("A CLI for managing Emacs time capsules")
        .arg(
            Arg::with_name("create_capsule")
                .short('c')
                .long("create_capsule")
                .help("Create a time capsule of the ~/.emacs directory"),
        )
        .arg(
            Arg::with_name("list_time_capsules")
                .short('l')
                .long("list_time_capsule")
                .help("List all available time capsules"),
        )
        .arg(
            Arg::with_name("restore_time_capsule")
                .short('r')
                .long("restore_time_capsule")
                .help("Restore a specific time capsule")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("help")
                .short('h')
                .long("help")
                .help("Displays help information"),
        )
        .arg(
            Arg::with_name("version")
                .short('v')
                .long("version")
                .help("Displays version information"),
        )
        .get_matches();

    // Handle CLI flags
    if matches.is_present("create_capsule") {
        create_time_capsule_fn();
    } else if matches.is_present("restore_time_capsule") {
        // Handle the error in the restore_time_capsule_fn
        restore_time_capsule_fn();
    } else if matches.is_present("list_time_capsules") {
        list_time_capsules_fn()
    }
}
// #################################################
//               CREATE TIME CAPSULE
// #################################################

fn create_time_capsule_fn() {
    println!("Creating time capsule...");

    // Get the current timestamp in the desired format
    let timestamp = Local::now().format("%a_%b_%d_%Y_%H_%M_%S").to_string();

    // Create the destination file path
    let destination_file_name = format!("emacs_capsule_{}.zip", timestamp);
    let destination_dir = dirs::home_dir()
        .expect("Failed to determine home directory")
        .join(".emacs_capsules");
    let destination_file = destination_dir.join(destination_file_name);

    // Check if the destination file already exists
    if destination_file.exists() {
        eprintln!("Destination file already exists: {:?}", destination_file);
        exit(1);
    }

    // Create the destination directory if it doesn't exist
    fs::create_dir_all(&destination_dir).expect("Failed to create destination directory");

    // Create the zip archive
    let file = File::create(&destination_file).expect("Failed to create destination file");
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Bzip2); // Deflated, Zstd

    let mut zip_writer = ZipWriter::new(file);

    // Get the source directory path
    let source_dir = dirs::home_dir()
        .expect("Failed to determine home directory")
        .join(".emacs.d");

    // Get the total number of files in the source directory
    let total_files = count_files(&source_dir);

    // Create a progress bar
    let progress_bar = ProgressBar::new(total_files);
    progress_bar.set_style(
        ProgressStyle::default_bar().template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
        ),
    );

    // Walk through the source directory and add files to the zip archive
    let result = walk_dir(&source_dir, &mut zip_writer, &options, &progress_bar);

    // Finish writing the zip archive
    zip_writer
        .finish()
        .expect("Failed to write the zip archive");

    // Finish the progress bar
    progress_bar.finish();

    if result.is_ok() {
        println!("Time capsule created at: {:?}", destination_file);
    } else {
        eprintln!("Failed to create time capsule");
        eprintln!("Error: {:?}", result.err());
        exit(1);
    }
}

fn count_files(dir: &Path) -> u64 {
    let mut count = 0;
    for entry in dir.read_dir().unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            count += count_files(&path);
        } else {
            count += 1;
        }
    }
    count
}

fn walk_dir(
    dir: &Path,
    zip_writer: &mut ZipWriter<File>,
    options: &FileOptions,
    progress_bar: &ProgressBar,
) -> io::Result<()> {
    for entry in dir.read_dir()? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Create a new directory entry in the zip archive
            let dir_name = match path.strip_prefix(&dirs::home_dir().unwrap().join(".emacs.d")) {
                Ok(stripped) => stripped.to_string_lossy().into_owned(),
                Err(err) => {
                    eprintln!("Failed to strip prefix for directory: {:?}", err);
                    continue;
                }
            };
            let zip_dir_path = format!("emacs.d/{}", dir_name);
            zip_writer.add_directory(zip_dir_path, Default::default())?;

            walk_dir(&path, zip_writer, options, progress_bar)?;
        } else {
            let file_name = match path.strip_prefix(&dirs::home_dir().unwrap().join(".emacs.d")) {
                Ok(stripped) => stripped.to_string_lossy().into_owned(),
                Err(err) => {
                    eprintln!("Failed to strip prefix for file: {:?}", err);
                    continue;
                }
            };

            let mut file = File::open(&path)?;
            zip_writer.start_file(file_name, *options)?;
            io::copy(&mut file, zip_writer)?;
            progress_bar.inc(1);
        }
    }

    Ok(())
}

// #################################################
//               RESTORE TIME CAPSULE
// #################################################

fn extract_file(
    zip_file: &mut zip::read::ZipFile,
    dest_path: &Path,
    progress_bar: &ProgressBar,
) -> io::Result<()> {
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut dest_file = fs::File::create(dest_path)?;
    io::copy(zip_file, &mut dest_file)?;

    progress_bar.inc(1);

    Ok(())
}

#[allow(deprecated)]
fn restore_time_capsule_fn() -> io::Result<()> {
    // Check if ~/.emacs.d directory exists
    let emacs_dir = dirs::home_dir().unwrap().join(".emacs.d");
    let backup_dir = format!(
        "{}/.emacs.backup_{}",
        emacs_dir.parent().unwrap().to_string_lossy(),
        chrono::Local::now().format("%Y%m%d%H%M%S")
    );
    if emacs_dir.exists() {
        // Move ~/.emacs.d to ~/.emacs.y_m_d_h_m_s
        fs::rename(&emacs_dir, &backup_dir)?;
    }

    // List capsules in ~/.emacs_capsules directory
    let capsules_dir = dirs::home_dir().unwrap().join(".emacs_capsules");

    if capsules_dir.exists() {
        let entries = fs::read_dir(capsules_dir)?;

        // Store the list of capsule files
        let mut capsule_files: Vec<PathBuf> = Vec::new();

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.file_name() != Some(std::ffi::OsStr::new(".DS_Store")) {
                capsule_files.push(path);
            }
        }

        // Sort the capsule files chronologically
        capsule_files.sort();

        // Display the list of capsules
        println!("Available Capsules:");
        for (index, capsule) in capsule_files.iter().enumerate() {
            //println!("{}: {:?}", index + 1, capsule.file_name().unwrap());
            println!(
                "\x1b[93m[\u{f2da} ]\x1b[0m:(\x1b[96m{}\x1b[0m): {:?}",
                index + 1,
                capsule.file_name().unwrap()
            );
        }

        // Prompt the user to select a capsule
        let mut input = String::new();
        println!("Select a capsule to restore (enter the number):");
        io::stdin().read_line(&mut input)?;
        let selected_index: usize = input.trim().parse().unwrap();
        let selected_capsule = &capsule_files[selected_index - 1];

        // Extract the selected capsule to ~/.emacs.d
        let file = File::open(selected_capsule)?;
        let mut zip = ZipArchive::new(file)?;

        // Create a progress bar
        let progress_bar = ProgressBar::new(zip.len() as u64);
        progress_bar.set_style(ProgressStyle::default_bar().template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
        ));

        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            let file_path = file.sanitized_name();
            let dest_path = emacs_dir.join(
                file_path
                    .strip_prefix(".emacs.d")
                    .unwrap_or_else(|_| file_path.as_ref()),
            );

            if file.is_dir() {
                fs::create_dir_all(&dest_path)?;
            } else {
                extract_file(&mut file, &dest_path, &progress_bar)?;
            }
        }
        // Finish the progress bar
        progress_bar.finish();
        println!("Restoration complete.");
    } else {
        println!("No capsules found in ~/.emacs_capsules directory.");
    }

    Ok(())
}

// #################################################
//               LIST ALL TIME CAPSULES
//          List all capsules in our storage
// #################################################

fn list_time_capsules_fn() {
    // Check if ~/.emacs_capsules directory exists
    let capsules_dir = dirs::home_dir().unwrap().join(".emacs_capsules");

    if capsules_dir.exists() {
        // Read the contents of the directory
        let entries = fs::read_dir(capsules_dir).unwrap();

        // Store the list of capsule files
        let capsule_files: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let file_name = entry.file_name().to_string_lossy().to_string();
                !file_name.contains(".DS_Store")
            })
            .filter(|entry| entry.path().is_file())
            .map(|entry| entry.path())
            .collect();

        // Sort capsule files chronologically (newer on top)
        let mut sorted_capsules = capsule_files;
        sorted_capsules.sort_by(|a, b| {
            let a_metadata = fs::metadata(a).unwrap();
            let b_metadata = fs::metadata(b).unwrap();
            let a_modified = a_metadata.modified().unwrap();
            let b_modified = b_metadata.modified().unwrap();
            b_modified.cmp(&a_modified)
        });

        // Display the list of capsules
        println!("Available Capsules:");
        for (index, capsule) in sorted_capsules.iter().enumerate() {
            println!(
                "\x1b[93m[\u{f2da} ]\x1b[0m:(\x1b[96m{}\x1b[0m): {:?}",
                index + 1,
                capsule.file_name().unwrap()
            );
        }
    } else {
        println!("No capsules found in ~/.emacs_capsules directory.");
    }
}
// List capsules in our storage
