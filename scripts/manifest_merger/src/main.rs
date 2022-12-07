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
use git2::{Error, Repository};
use manifest::Manifest;
use regex::Regex;
use reqwest::Client;
use std::fs;
use std::option::Option;

mod git;
#[macro_use]
mod macros;
mod manifest;
mod merge;

const FLAMINGO_VENDOR: &str = "vendor/flamingo";
const VERSION_FILE: &str = "target/product/version.mk";
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
async fn main() -> Result<(), String> {
    let args = Args::parse();

    if !args.system_tag.is_some() && !args.vendor_tag.is_some() {
        return Err(String::from(
            "No tags specified. Specify atleast one of -s or -v",
        ));
    }

    let system_manifest = args
        .system_tag
        .as_ref()
        .map(|tag| Manifest::new(&args.mainfest_dir, "system", Some(tag.to_owned())));
    let vendor_manifest = args
        .vendor_tag
        .as_ref()
        .map(|tag| Manifest::new(&args.mainfest_dir, "vendor", Some(tag.to_owned())));

    let client = Client::new();

    let (system_update, vendor_update) = futures::join!(
        manifest::update(&client, &system_manifest),
        manifest::update(&client, &vendor_manifest)
    );
    system_update?;
    vendor_update?;

    let default_manifest = Manifest::new(&args.mainfest_dir, "default", None);
    manifest::update_default(default_manifest, &system_manifest, &vendor_manifest, args.push)?;

    let flamingo_manifest = Manifest::new(&args.mainfest_dir, "flamingo", None);
    merge::merge_upstream(
        &args.source_dir,
        flamingo_manifest,
        &system_manifest,
        &vendor_manifest,
        args.threads,
        args.push
    )?;

    if args.set_version.is_some() {
        let (major, minor) = args
            .set_version
            .unwrap()
            .split_once('.')
            .map(|(major, minor)| major.parse::<usize>().ok().zip(minor.parse::<usize>().ok()))
            .flatten()
            .ok_or(String::from("--set-version value is malformed"))?;
        set_version(major, minor, &args.source_dir, args.push)?;
    }

    update_manifest(
        &args.mainfest_dir,
        &args.system_tag,
        &args.vendor_tag,
        args.push,
    )
    .map_err(|err| format!("Failed to update manifest: {err}"))
}

fn update_manifest(
    mainfest_dir: &str,
    system_tag: &Option<String>,
    vendor_tag: &Option<String>,
    push: bool,
) -> Result<(), Error> {
    let repo = Repository::open(mainfest_dir)?;
    git::get_or_create_remote(&repo, MANIFEST_REMOTE_NAME, MANIFEST_REMOTE_URL)?;
    let mut message = format!("manifest: upstream with clo\n");
    if let Some(tag) = system_tag {
        message = format!("{message}\n* system tag: {tag}");
    }
    if let Some(tag) = vendor_tag {
        message = format!("{message}\n* vendor tag: {tag}");
    }
    git::add_and_commit(&repo, ".", &message)?;
    if push {
        git::push(&repo)
    } else {
        Ok(())
    }
}

fn set_version(
    major_version: usize,
    minor_version: usize,
    source: &str,
    push: bool,
) -> Result<(), String> {
    let file = format!("{source}/{FLAMINGO_VENDOR}/{VERSION_FILE}");
    let version_file_content =
        fs::read_to_string(&file).map_err(|err| format!("Failed to read version file: {err}"))?;

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

    fs::write(file, version_file_content.to_string())
        .map_err(|err| format!("Failed to set version: {err}"))?;

    let repo_path = format!("{source}/{FLAMINGO_VENDOR}");
    let repo = Repository::open(&repo_path)
        .map_err(|err| format!("Failed to open {FLAMINGO_VENDOR} repository: {err}"))?;
    let message = format!(
        "flamingo: version: update to {}.{}",
        major_version, minor_version
    );
    git::add_and_commit(&repo, VERSION_FILE, &message)
        .map_err(|err| format!("Failed to commit version change: {err}"))?;
    if push {
        git::push(&repo).map_err(|err| format!("Failed to push {FLAMINGO_VENDOR} repo: {err}"))
    } else {
        Ok(())
    }
}
