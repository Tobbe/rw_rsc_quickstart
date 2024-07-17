use serde_json::Map;
use serde_json::Value;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;

fn main() {
    if !Path::new("template").exists() {
        let url = "https://github.com/redwoodjs/redwood/archive/refs/heads/main.zip";
        let resp = reqwest::blocking::get(url).expect("request failed");
        let archive = resp.bytes().expect("body invalid");

        let target_dir = get_tempdir();
        println!("Extracting into {}", target_dir.to_string_lossy());

        // The third parameter allows you to strip away toplevel directories.
        // If `archive` contained a single directory, its contents would be extracted instead.
        zip_extract::extract(Cursor::new(archive), &target_dir, true)
            .expect("Failed to extract zip");

        let from = target_dir
            .join("__fixtures__")
            .join("test-project-rsc-kitchen-sink");

        fs::rename(from, "template").expect("Failed to rename");

        fs::remove_dir_all(target_dir).expect("Failed to remove temp dir");
    }

    let latest_rw_canary = get_latest_canary("@redwoodjs/core");

    let package_jsons = glob::glob("template/**/package.json").expect("Failed to glob");

    for entry in package_jsons {
        let path = entry.expect("Failed to get path");

        println!(
            "Updating {} to use latest RW canary version",
            path.to_string_lossy()
        );

        let contents = fs::read_to_string(&path).expect("Failed to read file");

        let mut json: serde_json::Value =
            serde_json::from_str(&contents).expect("Failed to parse json");

        let dependencies = match json["dependencies"].as_object_mut() {
            Some(deps) => deps,
            None => &mut Map::new(),
        };

        for (name, value) in dependencies.iter_mut() {
            if name.starts_with("@redwoodjs/") {
                *value = Value::String(latest_rw_canary.clone());
            }
        }

        let dev_dependencies = match json["devDependencies"].as_object_mut() {
            Some(dev_deps) => dev_deps,
            None => &mut Map::new(),
        };

        for (name, value) in dev_dependencies.iter_mut() {
            if name.starts_with("@redwoodjs/") {
                *value = Value::String(latest_rw_canary.clone());
            }
        }

        fs::write(&path, json.to_string()).expect("Failed to write file");
    }
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

    packument.pointer("/dist-tags/canary").unwrap().to_string()
}
