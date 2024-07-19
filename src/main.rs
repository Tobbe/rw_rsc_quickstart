use clap::Parser;
use lazy_static::lazy_static;
use semver_rs::satisfies;
use serde_json::Value;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use std::sync::RwLock;

lazy_static! {
    static ref CONFIG: RwLock<Config> = RwLock::new(Config { verbose: false });
}

struct Config {
    verbose: bool,
}

impl Config {
    fn set_verbose(verbose: bool) {
        let mut config = CONFIG.write().unwrap();
        config.verbose = verbose;
    }

    fn is_verbose() -> bool {
        let config = CONFIG.read().unwrap();
        config.verbose
    }
}

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

    if args.verbose {
        println!("{:?}", args);
    }

    Config::set_verbose(args.verbose);

    check_node();
    check_yarn_installation();

    if !Path::new(&args.installation_dir).exists() {
        let url = "https://github.com/redwoodjs/redwood/archive/refs/heads/main.zip";
        let resp = reqwest::blocking::get(url).expect("request failed");
        let archive = resp.bytes().expect("body invalid");

        let target_dir = get_tempdir();

        if Config::is_verbose() {
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
    if Config::is_verbose() {
        println!("Latest canary: {latest_rw_canary}");
    }

    // TODO: Just hard-code the paths. We know what they are.
    let package_jsons =
        glob::glob(&format!("{}/**/package.json", args.installation_dir)).expect("Failed to glob");

    update_package_jsons(package_jsons, latest_rw_canary);

    println!("Checking your yarn version");
    check_yarn_version(&args.installation_dir);

    println!("Running `yarn install`. This might take a while...");
    exec_in("yarn install", &args.installation_dir);

    println!("Initializing git");
    exec_in("git init .", &args.installation_dir);
    exec_in("git add .", &args.installation_dir);
    exec_in("git commit -am 'Initial commit'", &args.installation_dir);

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

fn update_package_jsons(package_jsons: glob::Paths, latest_rw_canary: String) {
    for entry in package_jsons {
        let path = entry.expect("Failed to get path");

        if Config::is_verbose() {
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

fn check_node() {
    let output = exec("node --version");
    let version = output.trim();

    if Config::is_verbose() {
        println!("Node version: {version}");
    }

    // Compare semver versions. Node has to be at least v 20
    if !(satisfies(version, ">=20", None).unwrap()) {
        eprintln!("Your Node version is too old. Please install Node v20 or newer");
        std::process::exit(1);
    }
}

fn check_yarn_installation() {
    let yarn = match which::which("yarn") {
        Ok(path) => path,
        Err(_) => {
            eprintln!("Could not find `yarn`");
            eprintln!("Please enable yarn by running `corepack enable`");
            eprintln!("and then upgrade by running `corepack install --global yarn@latest`");
            std::process::exit(1);
        }
    };

    if Config::is_verbose() {
        println!("Yarn path: {}", yarn.to_string_lossy());
    }

    let yarn = fs::canonicalize(yarn).expect("Failed to canonicalize path");

    let yarn_path_str = yarn.to_string_lossy();

    if Config::is_verbose() {
        println!("Yarn canonical path: {}", yarn_path_str);
        println!("Running {} --version", yarn_path_str);
    }

    if yarn_path_str.contains("/corepack/") || yarn_path_str.contains("\\corepack\\") {
        // The first found `yarn` seems to be installed by corepack, so all is good
        return;
    }

    // If we get this far in the code we know there is at least one yarn, so
    // it's safe to just unwrap() here
    let all_yarns = which::which_all("yarn").unwrap();

    let mut count = 0;
    let mut has_corepack_yarn = false;

    for yarn in all_yarns {
        let yarn = fs::canonicalize(yarn).expect("Failed to canonicalize path");
        let yarn_path_str = yarn.to_string_lossy();

        if Config::is_verbose() {
            println!("Found yarn: {}", yarn_path_str);
        }

        count += 1;

        if yarn_path_str.contains("/corepack/") || yarn_path_str.contains("\\corepack\\") {
            has_corepack_yarn = true;
        }
    }

    if Config::is_verbose() {
        println!("Number of yarn found in PATH: {count}")
    }

    if has_corepack_yarn {
        eprintln!("You have more than one active yarn installation");
        eprintln!("Perhaps you've manually installed it using Homebrew or npm");
        eprintln!("Please completely uninstall yarn and then enable it using corepack.");
        eprintln!("The only correct way to enable yarn is by running");
        eprintln!("`corepack enable`");
        eprintln!("(yarn is already shipped with Node, you just need to enable it)");
        std::process::exit(1);
    }

    if count > 1 {
        eprintln!(
            "Multiple yarn binaries found. This could be a problem. Make sure \
            the first `yarn` in your PATH is the one you want to use."
        );
        std::process::exit(1);
    }
}

fn check_yarn_version(installation_dir: &str) {
    let output = exec_in("yarn --version", installation_dir);
    let yarn_version = output.trim();

    if Config::is_verbose() {
        println!("Yarn version: {yarn_version}");
    }

    // Compare semver versions. Yarn should be at least v4
    // TODO: Read packageManager from package.json and compare exactly with that version
    if !(satisfies(yarn_version, ">=4", None).unwrap()) {
        eprintln!(
            "Something is wrong with your yarn installation. It should have \
            picked up on the `packageManager` field in `package.json` and \
            upgraded itself to the required version"
        );
        std::process::exit(1);
    }
}

fn exec<S: Into<String>>(cmd: S) -> String {
    exec_with_optional_cwd(cmd, None)
}

fn exec_in<S: Into<String>, P: AsRef<Path>>(cmd: S, cwd: P) -> String {
    exec_with_optional_cwd(cmd, Some(cwd.as_ref()))
}

/// Internal function to execute a command with an optional current working
/// directory
/// Prefer `exec` or `exec_in` instead of this function for actual usage in the
/// code as they provide a more ergonomic interface
fn exec_with_optional_cwd<S: Into<String>>(cmd: S, cwd_option: Option<&Path>) -> String {
    // rustc knows that cmd_string is a String, but the Rust language server
    // doesn't, so I'm helping it along here by explicitly annotating the type
    let cmd_string: String = cmd.into();
    let mut cmd_parts = cmd_string.split_whitespace();
    let cmd = cmd_parts.next().expect("No command provided");

    let mut command = std::process::Command::new(cmd);
    command.args(cmd_parts);

    if let Some(cwd) = cwd_option {
        command.current_dir(cwd);
    }

    let output = command.output().expect("Failed to execute command");

    if !output.status.success() {
        eprintln!("`{cmd}` exited with code {}", output.status.code().unwrap());
        std::process::exit(1);
    }

    let output = String::from_utf8(output.stdout).expect("Failed to parse output");

    if Config::is_verbose() {
        println!("`{cmd}` output:");
        println!("{output}");
    }

    output
}
