/*
 * Copyright (C) 2022 FlamingoOS Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use clap::Parser;
use git2::Repository;
use manifest::Manifest;
use regex::Regex;
use reqwest::Client;
use std::fs;
use std::option::Option;
use std::process;

mod git;
#[macro_use]
mod macros;
mod manifest;
mod merge;

const VERSION_FILE_PATH: &str = "vendor/flamingo/target/product/version.mk";
const MAJOR_VERSION_STR: &str = "FLAMINGO_VERSION_MAJOR";
const MINOR_VERSION_STR: &str = "FLAMINGO_VERSION_MINOR";

const MANIFEST_REMOTE_NAME: &str = "flamingo";
const MANIFEST_REMOTE_URL: &str = "ssh://git@github.com/Flamingo-OS/manifest";

#[derive(Parser)]
struct Args {
    /// Source directory of the rom
    #[arg(long, default_value_t = String::from("./"))]
    source_dir: String,

    /// Location of the manifest dir
    #[arg(short, long, default_value_t = String::from("./.repo/manifests"))]
    mainfest_dir: String,

    /// CLO system tag that should be merged across the rom
    #[arg(short, long)]
    system_tag: Option<String>,

    /// CLO system tag that should be merged across the rom
    #[arg(short, long)]
    vendor_tag: Option<String>,

    /// Number of threads to use.
    #[arg(short, long, default_value_t = num_cpus::get())]
    threads: usize,

    /// Whether to push the changes to the remote
    #[arg(short, long, default_value_t = false)]
    push: bool,

    /// Version to be set
    #[arg(long)]
    set_version: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if !args.system_tag.is_some() && !args.vendor_tag.is_some() {
        error!("No tags specified. Specify atleast one of -s or -v");
        process::exit(1);
    }

    let system_manifest = args
        .system_tag
        .map(|tag| Manifest::new(&args.mainfest_dir, "system", Some(tag)));
    let vendor_manifest = args
        .vendor_tag
        .map(|tag| Manifest::new(&args.mainfest_dir, "vendor", Some(tag)));

    let client = Client::new();

    futures::join!(
        manifest::update(&client, &system_manifest),
        manifest::update(&client, &vendor_manifest)
    );

    let default_manifest = Manifest::new(&args.mainfest_dir, "default", None);
    manifest::update_default(default_manifest, &system_manifest, &vendor_manifest);

    let flamingo_manifest = Manifest::new(&args.mainfest_dir, "flamingo", None);
    merge::merge_upstream(
        &args.source_dir,
        flamingo_manifest,
        &system_manifest,
        &vendor_manifest,
        args.threads,
        args.push,
    );

    // Push manifest repo if everything went well.
    if args.push {
        match Repository::open(&args.mainfest_dir) {
            Ok(repo) => {
                let result =
                    git::get_or_create_remote(&repo, MANIFEST_REMOTE_NAME, MANIFEST_REMOTE_URL);
                if let Err(err) = result {
                    error_exit!("{}", err);
                }
                if let Err(err) = git::push(&repo, MANIFEST_REMOTE_NAME) {
                    error_exit!("failed to push manifest: {err}");
                }
            }
            Err(err) => {
                error_exit!("failed to open manifest repository: {err}");
            }
        }
    }

    if args.set_version.is_some() {
        let version = args.set_version.unwrap();
        let mut version_itr = version.split('.');
        set_version(
            version_itr.next().unwrap().parse().unwrap(),
            version_itr.next().unwrap().parse().unwrap(),
            &args.source_dir,
        );
    }
}

fn set_version(major_version: usize, minor_version: usize, source: &str) {
    let file = format!("{source}/{VERSION_FILE_PATH}");
    let version_file_content = fs::read_to_string(&file).expect("Failed to read version file");

    let regex = Regex::new(r"FLAMINGO_VERSION_MAJOR\s:=\s\d+").unwrap();
    let version_file_content = regex.replace(
        &version_file_content,
        format!("{} := {}", MAJOR_VERSION_STR, major_version),
    );

    let regex = Regex::new(r"FLAMINGO_VERSION_MINOR\s:=\s\d+").unwrap();
    let version_file_content = regex.replace(
        &version_file_content,
        format!("{} := {}", MINOR_VERSION_STR, minor_version),
    );

    fs::write(file, version_file_content.to_string()).expect("Failed to set version");
}
