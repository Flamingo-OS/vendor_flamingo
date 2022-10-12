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
use errors::error;
use manifest::Manifest;
use regex::Regex;
use reqwest::Client;
use std::fs;
use std::option::Option;
use std::process;

use crate::{
    manifest::{update_default_manifest, update_manifests},
    merge::merge_upstream,
};

mod errors;
mod manifest;
mod merge;

const ATTR_NAME: &str = "name";
const ATTR_PATH: &str = "path";

const VERSION_FILE_PATH: &str = "vendor/flamingo/target/product/version.mk";
const MAJOR_VERSION_STR: &str = "FLAMINGO_VERSION_MAJOR";
const MINOR_VERSION_STR: &str = "FLAMINGO_VERSION_MINOR";

struct Version {
    major: usize,
    minor: usize,
}

#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    /// Source directory of the rom
    #[arg(long, default_value_t = format!("./"))]
    source_dir: String,

    /// Location of the manifest dir
    #[arg(short, long, default_value_t = format!("./.repo/manifests"))]
    mainfest_dir: String,

    /// CLO system tag that should be merged across the rom
    #[arg(short, long)]
    system_tag: Option<String>,

    /// CLO system tag that should be merged across the rom
    #[arg(short, long)]
    vendor_tag: Option<String>,

    /// Number of threads to use.
    #[arg(short, long, default_value_t = num_cpus::get() * 2)]
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
        error("No tags specified. Specify atleast one of -s or -v");
        process::exit(1);
    }

    let system_manifest = args
        .system_tag
        .map(|tag| Manifest::new(args.mainfest_dir.clone(), "system", Some(tag)));
    let vendor_manifest = args
        .vendor_tag
        .map(|tag| Manifest::new(args.mainfest_dir.clone(), "vendor", Some(tag)));

    let client = Client::new();

    futures::join!(
        update_manifests(client.clone(), &system_manifest),
        update_manifests(client.clone(), &vendor_manifest)
    );

    let default_manifest = Manifest::new(args.mainfest_dir.clone(), "default", None);
    update_default_manifest(default_manifest, &system_manifest, &vendor_manifest);

    let flamingo_manifest = Manifest::new(args.mainfest_dir.clone(), "flamingo", None);
    merge_upstream(
        args.source_dir.clone(),
        flamingo_manifest,
        &system_manifest,
        &vendor_manifest,
        args.threads,
        args.push,
    );

    if args.set_version.is_some() {
        let version = args.set_version.unwrap();
        let mut vers = version.split('.');
        let version = Version {
            major: vers.nth(0).unwrap().parse().unwrap(),
            minor: vers.nth(1).unwrap().parse().unwrap(),
        };
        set_version(version, args.source_dir.clone());
    }
}

fn set_version(version: Version, source: String) {
    let file = format!("{source}/{VERSION_FILE_PATH}");
    let version_file_content = fs::read_to_string(&file).expect("Failed to open version file");

    let regex = Regex::new(r"FLAMINGO_VERSION_MAJOR\s:=\s\d+").unwrap();
    let version_file_content = regex.replace(
        &version_file_content,
        format!("{} := {}", MAJOR_VERSION_STR, version.major),
    );

    let regex = Regex::new(r"FLAMINGO_VERSION_MINOR\s:=\s\d+").unwrap();
    let version_file_content = regex.replace(
        &version_file_content,
        format!("{} := {}", MINOR_VERSION_STR, version.minor),
    );

    fs::write(file, version_file_content.to_string()).expect("Failed to set version");
}
