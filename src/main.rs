use clap::Parser;
use serde_json::Value;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;

/// Quick start for RedwoodJS with React Server Components
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Show verbose output
    #[arg(short, long)]
    verbose: bool,
    /// Where you want to create the project
    #[arg(value_parser = clap::builder::NonEmptyStringValueParser::new())]
    installation_dir: String,
}

fn main() {
    let args = Args::parse();
    println!("{:?}", args);

    if !Path::new(&args.installation_dir).exists() {
        let url = "https://github.com/redwoodjs/redwood/archive/refs/heads/main.zip";
        let resp = reqwest::blocking::get(url).expect("request failed");
        let archive = resp.bytes().expect("body invalid");

        let target_dir = get_tempdir();

        if args.verbose {
            println!("Extracting into {}", target_dir.to_string_lossy());
        }

        // The third parameter allows you to strip away toplevel directories.
        // If `archive` contained a single directory, its contents would be extracted instead.
        zip_extract::extract(Cursor::new(archive), &target_dir, true)
            .expect("Failed to extract zip");

        let from = target_dir
            .join("__fixtures__")
            .join("test-project-rsc-kitchen-sink");

        fs::rename(from, &args.installation_dir).expect("Failed to rename");

        fs::remove_dir_all(target_dir).expect("Failed to remove temp dir");
    }

    let latest_rw_canary = get_latest_canary("@redwoodjs/core");
    if args.verbose {
        println!("Latest canary: {latest_rw_canary}");
    }

    // TODO: Just hard-code the paths. We know what they are.
    let package_jsons =
        glob::glob(&format!("{}/**/package.json", args.installation_dir)).expect("Failed to glob");

    update_package_jsons(package_jsons, latest_rw_canary, args.verbose);

    println!(
        "Done! You can now run `yarn install` in the `{}` directory.",
        args.installation_dir
    );
}

fn get_tempdir() -> PathBuf {
    tempfile::Builder::new()
        .prefix("rwjs-rsc-quickstart-")
        .rand_bytes(12)
        .tempdir()
        .unwrap()
        .into_path()
}

fn get_latest_canary<S: Into<String>>(package: S) -> String {
    let url = "https://registry.npmjs.org/".to_string() + &package.into();
    let resp = reqwest::blocking::get(url).expect("request failed");
    let packument: serde_json::Value = resp.json().expect("body invalid");

    packument
        .pointer("/dist-tags/canary")
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}

fn update_package_jsons(package_jsons: glob::Paths, latest_rw_canary: String, verbose: bool) {
    for entry in package_jsons {
        let path = entry.expect("Failed to get path");

        if verbose {
            println!(
                "Updating {} to use latest RW canary version",
                path.to_string_lossy()
            );
        }

        let contents = fs::read_to_string(&path).expect("Failed to read file");

        let mut json: serde_json::Value =
            serde_json::from_str(&contents).expect("Failed to parse json");

        if json.get("dependencies").is_some() {
            let dependencies = json["dependencies"].as_object_mut().unwrap();

            for (name, value) in dependencies.iter_mut() {
                if name.starts_with("@redwoodjs/") {
                    *value = Value::String(latest_rw_canary.clone());
                }
            }
        }

        if json.get("devDependencies").is_some() {
            let dev_dependencies = json["devDependencies"].as_object_mut().unwrap();

            for (name, value) in dev_dependencies.iter_mut() {
                if name.starts_with("@redwoodjs/") {
                    *value = Value::String(latest_rw_canary.clone());
                }
            }
        }

        let pretty_json = serde_json::to_string_pretty(&json).expect("Failed to serialize json");
        fs::write(&path, format!("{pretty_json}\n")).expect("Failed to write file");
    }
}
