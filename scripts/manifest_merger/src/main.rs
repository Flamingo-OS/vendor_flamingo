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

use colored::Colorize;
use git2::{Cred, Error, ErrorCode, PushOptions, Remote, RemoteCallbacks, Repository};
use manifest::{CloManifest, Manifest, ManifestFmt};
use regex::Regex;
use reqwest::Client;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fmt::Display;
use std::fs;
use std::io::{BufReader, Read};
use std::option::Option;
use std::process;
use std::vec::Vec;
use threadpool::ThreadPool;
use xmltree::{Element, EmitterConfig, XMLNode};

pub mod manifest;

const ELEMENT_MANIFEST: &str = "manifest";
const ELEMENT_PROJECT: &str = "project";

const ATTR_REMOTE: &str = "remote";
const ATTR_NAME: &str = "name";
const ATTR_PATH: &str = "path";
const ATTR_REVISION: &str = "revision";
const ATTR_CLONE_DEPTH: &str = "clone-depth";

const XML_INDENT: &str = "    ";

const FLAMINGO_REMOTE: &str = "flamingo";
const FLAMINGO_BRANCH: &str = "A13";

const VERSION_FILE_PATH: &str = "vendor/flamingo/target/product/version.mk";
const MAJOR_VERSION_STR: &str = "FLAMINGO_VERSION_MAJOR";
const MINOR_VERSION_STR: &str = "FLAMINGO_VERSION_MINOR";

struct Version {
    major: usize,
    minor: usize,
}

#[tokio::main]
async fn main() {
    let mut source_directory: Option<String> = Option::None;
    let mut manifest_directory: Option<String> = Option::None;
    let mut system_tag: Option<String> = Option::None;
    let mut vendor_tag: Option<String> = Option::None;
    let mut thread_count = 1;
    let mut push = false;
    let mut version: Option<Version> = Option::None;
    parse_args(
        &mut source_directory,
        &mut manifest_directory,
        &mut system_tag,
        &mut vendor_tag,
        &mut thread_count,
        &mut push,
        &mut version,
    );

    // No-op w/o source / manifest directory
    if source_directory.is_none() {
        error_and_exit("source directory not specified, exiting");
    }
    if manifest_directory.is_none() {
        error_and_exit("manifest directory not specified, exiting");
    }

    // Filter child elements of <manifest></manifest>
    // Currently we only care about <project> elements.
    let elements_to_keep = HashSet::from([String::from(ELEMENT_PROJECT)]);

    // Remove attributes from <project> elements.
    let attrs_to_keep = HashSet::from([
        String::from(ATTR_CLONE_DEPTH),
        String::from(ATTR_NAME),
        String::from(ATTR_PATH),
    ]);

    // Shallow clone (clone-depth="1") some big repos by default
    // to save space in machine.
    let shallow_clone_repos = HashSet::from([
        String::from("platform/external/"),
        String::from("platform/prebuilts/"),
    ]);

    let manifest_directory = manifest_directory.unwrap();
    let system_manifest = system_tag.map(|tag| CloManifest::system(&manifest_directory, tag));
    let vendor_manifest = vendor_tag.map(|tag| CloManifest::vendor(&manifest_directory, tag));

    let client = Client::new();

    if system_manifest.is_none() && vendor_manifest.is_none() {
        error("at least one of system or vendor tag is expected");
        help_and_exit();
    }
    futures::join!(
        update_manifests(
            &client,
            &system_manifest,
            &elements_to_keep,
            &attrs_to_keep,
            &shallow_clone_repos
        ),
        update_manifests(
            &client,
            &vendor_manifest,
            &elements_to_keep,
            &attrs_to_keep,
            &shallow_clone_repos
        )
    );

    let default_manifest = Manifest::default(&manifest_directory);
    update_default_manifest(default_manifest, &system_manifest, &vendor_manifest);

    let source_directory = source_directory.unwrap();
    let flamingo_manifest = Manifest::flamingo(&manifest_directory);
    merge_upstream(
        &source_directory,
        flamingo_manifest,
        &system_manifest,
        &vendor_manifest,
        thread_count,
        push,
    );
    if let Some(version) = version {
        set_version(version, &source_directory);
    }
}

fn parse_args(
    source_directory: &mut Option<String>,
    manifest_directory: &mut Option<String>,
    system_tag: &mut Option<String>,
    vendor_tag: &mut Option<String>,
    thread_count: &mut usize,
    push: &mut bool,
    version: &mut Option<Version>,
) {
    let args: Vec<String> = env::args().collect();
    let len = args.len();

    if len <= 1 {
        help_and_exit();
    }

    let mut i = 1;
    while i < len {
        let arg = &(*args[i])[..];
        match arg {
            "-h" | "--help" => help_and_exit(),
            "--source-dir" => {
                *source_directory = Some(
                    args.get(i + 1)
                        .expect("Directory path should be specified with {arg}")
                        .to_owned(),
                );
                i += 2;
            }
            "--manifest-dir" => {
                *manifest_directory = Some(
                    args.get(i + 1)
                        .expect("Directory path should be specified with {arg}")
                        .to_owned(),
                );
                i += 2;
            }
            "--system-tag" => {
                *system_tag = Some(
                    args.get(i + 1)
                        .expect("Tag should be specified with {arg}")
                        .to_owned(),
                );
                i += 2;
            }
            "--vendor-tag" => {
                *vendor_tag = Some(
                    args.get(i + 1)
                        .expect("Tag should be specified with {arg}")
                        .to_owned(),
                );
                i += 2;
            }
            "-t" | "--threads" => {
                *thread_count = args
                    .get(i + 1)
                    .expect("Thread count should be specified with {arg}")
                    .parse()
                    .expect("Thread count should be an integer");
                i += 2;
            }
            "-p" | "--push" => {
                *push = true;
                i += 1;
            }
            "--set-version" => {
                let raw_version = args
                    .get(i + 1)
                    .expect("Version should be specified with {arg}")
                    .to_owned();
                let mut iter = raw_version.split('.');
                *version = Some(Version {
                    major: iter.next().unwrap().parse().unwrap(),
                    minor: iter.next().unwrap().parse().unwrap(),
                });
                i += 2;
            }
            other => error_and_exit(&format!("unrecognised argument {other}")),
        }
    }
}

fn help_and_exit() {
    println!(
        "
Usage: manifest_merger [OPTIONS]

Options:
    -h, --help      Display this message
    --source-dir    Root directory of the source.
    --manifest-dir  Directory in which the manifest should be generated or updated.
    --system-tag    CLO tag for the system manifest.
    --vendor-tag    CLO tag for the vendor manifest.
    -t, --threads   Number of threads with which merge should be done. Defaults to 1.
    -p, --push      Push to remote repositories after merge is done.
    --set-version   Set flamingo version. Used when spl bump happens.
"
    );
    process::exit(0);
}

fn error(msg: &str) {
    eprintln!("{}", format!("Error: {msg}").red());
}

fn error_and_exit(msg: &str) {
    error(msg);
    process::exit(1);
}

async fn update_manifests(
    client: &Client,
    manifest: &Option<CloManifest>,
    elements_to_keep: &HashSet<String>,
    attrs_to_keep: &HashSet<String>,
    shallow_clone_repos: &HashSet<String>,
) {
    if manifest.is_some() {
        let manifest = manifest.as_ref().unwrap();
        let result = download_manifest(
            client,
            manifest,
            elements_to_keep,
            attrs_to_keep,
            shallow_clone_repos,
        )
        .await;
        match result {
            Ok(xml_manifest) => {
                let config = EmitterConfig::new()
                    .indent_string(XML_INDENT)
                    .perform_indent(true);
                if let Err(err) = xml_manifest.write_with_config(manifest.get_file(), config) {
                    error_and_exit(&format!("failed to write manifest: {}", err));
                }
            }
            Err(err) => {
                error_and_exit(&format!("failed to get manifest: {}", err));
            }
        }
    }
}

async fn download_manifest(
    client: &Client,
    manifest: &CloManifest,
    elements_to_keep: &HashSet<String>,
    attrs_to_keep: &HashSet<String>,
    shallow_clone_repos: &HashSet<String>,
) -> Result<Element, reqwest::Error> {
    let response = client.get(manifest.get_url()).send().await?;
    if !response.status().is_success() {
        error_and_exit(&format!(
            "GET request to {0} failed. Status code = {1}",
            manifest.get_url(),
            response.status().as_str()
        ));
    }
    let bytes = response.bytes().await.expect("Failed to get response body");
    let xml_manifest = Element::parse(&bytes[..]).expect("Failed to parse manifest");
    let new_xml_manifest = transform_manifest(
        xml_manifest,
        elements_to_keep,
        attrs_to_keep,
        shallow_clone_repos,
        &manifest.get_remote_name(),
    );
    Ok(new_xml_manifest)
}

fn transform_manifest(
    manifest: Element,
    elements_to_keep: &HashSet<String>,
    attrs_to_keep: &HashSet<String>,
    shallow_clone_repos: &HashSet<String>,
    remote: &String,
) -> Element {
    let mut transformed_manifest = Element::new(ELEMENT_MANIFEST);
    manifest
        .children
        .iter()
        .filter(|node| {
            if let XMLNode::Element(elem) = node {
                elements_to_keep.contains(&elem.name)
            } else {
                true
            }
        })
        .map(|node| {
            if let XMLNode::Element(elem) = node {
                let mut filtered_element = Element {
                    attributes: elem
                        .attributes
                        .iter()
                        .filter(|(key, _)| attrs_to_keep.contains(&key[..]))
                        .map(|(key, value)| (key.to_owned(), value.to_owned()))
                        .collect(),
                    ..elem.to_owned()
                };

                // Some repos have clone-depth="2", let's just keep
                // it 1 for our sake.
                filtered_element
                    .attributes
                    .entry(ATTR_CLONE_DEPTH.to_string())
                    .and_modify(|depth| *depth = String::from("1"));

                // Set remote from our default.xml manifest
                filtered_element
                    .attributes
                    .insert(ATTR_REMOTE.to_string(), remote.to_owned());

                let name = filtered_element.attributes.get(ATTR_NAME).unwrap();
                let should_shallow_clone = shallow_clone_repos
                    .iter()
                    .any(|prefix| name.starts_with(prefix));
                if should_shallow_clone {
                    filtered_element
                        .attributes
                        .entry(String::from(ATTR_CLONE_DEPTH))
                        .or_insert(String::from("1"));
                }
                XMLNode::Element(filtered_element)
            } else {
                node.to_owned()
            }
        })
        .for_each(|node| transformed_manifest.children.push(node));
    transformed_manifest
}

fn read_manifest<T: Display + ManifestFmt>(manifest: &T) -> Result<Element, String> {
    let mut bytes: Vec<u8> = Vec::new();
    let mut reader = BufReader::new(manifest.get_file());
    let read_result = reader.read_to_end(&mut bytes);
    match read_result {
        Ok(bytes_read) => {
            let parse_result = Element::parse(&bytes[..bytes_read]);
            match parse_result {
                Ok(element) => Ok(element),
                Err(err) => Err(format!("Failed to parse {manifest}: {err}")),
            }
        }
        Err(_) => Err(format!("Failed to read file {manifest}")),
    }
}

fn update_default_manifest(
    default_manifest: Manifest,
    system_manifest: &Option<CloManifest>,
    vendor_manifest: &Option<CloManifest>,
) {
    let mut xml_manifest =
        read_manifest(&default_manifest).expect("Failed to parse default manifest");
    xml_manifest.children.iter_mut().for_each(|node| {
        if let XMLNode::Element(elem) = node {
            if elem.name.eq(ATTR_REMOTE) {
                let remote_name = elem
                    .attributes
                    .get(ATTR_NAME)
                    .expect("Remote should have a name")
                    .clone();
                elem.attributes
                    .entry(ATTR_REVISION.to_string())
                    .and_modify(|revision| {
                        if system_manifest.is_some() {
                            let system_manifest = system_manifest.as_ref().unwrap();
                            if remote_name.eq(&system_manifest.get_remote_name()) {
                                *revision = system_manifest.get_revision();
                            }
                        } else if vendor_manifest.is_some() {
                            let vendor_manifest = vendor_manifest.as_ref().unwrap();
                            if remote_name.eq(&vendor_manifest.get_remote_name()) {
                                *revision = vendor_manifest.get_revision();
                            }
                        }
                    });
            }
        }
    });
}

fn get_repos<T: Display + ManifestFmt>(manifest: &T) -> Result<HashMap<String, String>, String> {
    read_manifest(manifest).map(|manifest| {
        manifest
            .children
            .iter()
            .map(|node| node.as_element())
            .filter(|element| element.is_some())
            .map(|element| element.unwrap())
            .filter(|element| {
                element.attributes.contains_key(ATTR_PATH)
                    && element.attributes.contains_key("name")
            })
            .map(|element| {
                let path = element.attributes.get(ATTR_PATH).unwrap().to_owned();
                let name = element.attributes.get(ATTR_NAME).unwrap().to_owned();
                (path, name)
            })
            .collect()
    })
}

struct MergeData {
    remote_name: String,
    remote_url: String,
    repo_path: String,
    repo_name: String,
    revision: String,
    push: bool,
}

fn merge_upstream(
    source: &str,
    flamingo_manifest: Manifest,
    system_manifest: &Option<CloManifest>,
    vendor_manifest: &Option<CloManifest>,
    thread_count: usize,
    push: bool,
) {
    let flamingo_repos = get_repos(&flamingo_manifest).unwrap();
    let system_repos = system_manifest
        .as_ref()
        .map_or(HashMap::new(), |manifest| get_repos(manifest).unwrap());
    let vendor_repos = vendor_manifest
        .as_ref()
        .map_or(HashMap::new(), |manifest| get_repos(manifest).unwrap());

    let thread_pool = ThreadPool::new(thread_count);
    flamingo_repos
        .iter()
        .map(|(path, _)| {
            if system_manifest.is_some() && system_repos.contains_key(&path[..]) {
                let system_manifest = system_manifest.as_ref().clone().unwrap();
                Some(MergeData {
                    remote_name: system_manifest.get_remote_name(),
                    remote_url: format!(
                        "{}/{}",
                        system_manifest.get_remote_url(),
                        system_repos.get(path).unwrap()
                    ),
                    repo_path: format!("{}/{}", source, path.to_string()),
                    repo_name: path.to_string(),
                    revision: system_manifest.get_revision(),
                    push: push,
                })
            } else if vendor_manifest.is_some() && vendor_repos.contains_key(&path[..]) {
                let vendor_manifest = vendor_manifest.as_ref().clone().unwrap();
                Some(MergeData {
                    remote_name: vendor_manifest.get_remote_name(),
                    remote_url: format!(
                        "{}/{}",
                        vendor_manifest.get_remote_url(),
                        vendor_repos.get(path).unwrap()
                    ),
                    repo_path: format!("{}/{}", source, path.to_string()),
                    repo_name: path.to_string(),
                    revision: vendor_manifest.get_revision(),
                    push: push,
                })
            } else {
                None
            }
        })
        .filter(|merge_data| merge_data.is_some())
        .map(|merge_data| merge_data.unwrap())
        .for_each(|merge_data| thread_pool.execute(|| merge_in_repo(merge_data)));
    thread_pool.join();
}

fn merge_in_repo(merge_data: MergeData) {
    println!("Merging in {}", &merge_data.repo_name);
    match Repository::open(&merge_data.repo_path) {
        Ok(repo) => {
            let result = repo.remote(&merge_data.remote_name, &merge_data.remote_url);
            let mut remote;
            if let Err(err) = result {
                if err.code() != ErrorCode::Exists {
                    error(&format!(
                        "failed to create remote {}: {err}",
                        &merge_data.remote_name
                    ));
                    return;
                }
                remote = repo.find_remote(&merge_data.remote_name).unwrap();
            } else {
                remote = result.unwrap();
            }
            if let Err(err) = fetch(&mut remote, &merge_data) {
                error(&err);
                return;
            }
            let reference = repo.find_reference(&merge_data.revision).unwrap();
            let annotated_commit = repo.reference_to_annotated_commit(&reference).unwrap();
            if let Err(err) = repo.merge(&[&annotated_commit], None, None) {
                error(&format!(
                    "failed to merge revision {} from {} in {} : {err}",
                    &merge_data.revision, &merge_data.remote_url, &merge_data.repo_name,
                ));
                return;
            }
            if !merge_data.push {
                return;
            }
            match push(&repo, &merge_data.repo_name) {
                Ok(_) => {
                    println!("Successfully pushed {}", &merge_data.repo_name);
                }
                Err(err) => {
                    error(&format!("failed to push {} : {err}", &merge_data.repo_name));
                }
            }
        }
        Err(err) => {
            error(&format!(
                "failed to open git repository at {}: {err}",
                &merge_data.repo_path
            ));
        }
    }
}

fn fetch(remote: &mut Remote, merge_data: &MergeData) -> Result<(), String> {
    remote
        .fetch(&[merge_data.revision.clone()], None, None)
        .map_err(|err| {
            format!(
                "failed to fetch revision {} from {} : {err}",
                &merge_data.revision, &merge_data.remote_url
            )
        })
}

fn push(repository: &Repository, name: &str) -> Result<(), Error> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_, username_from_url, _| {
        Cred::ssh_key_from_agent(&username_from_url.unwrap())
    });
    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);
    repository
        .find_remote(FLAMINGO_REMOTE)
        .expect(&format!("Flamingo remote not found in {name}"))
        .push(
            &[format!("HEAD:refs/heads/{FLAMINGO_BRANCH}")],
            Some(&mut push_options),
        )
}

fn set_version(version: Version, source: &str) {
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
