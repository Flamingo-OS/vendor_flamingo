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

/**
 * Note to maintainers:
 * Dependency file (json) should be formatted in the following manner:
 * [
 *     {
 *          "repository": "device_brand_name",
 *          "target_path": "device/brand/name",
 *          "remote": "flamingo",
 *          "revision": "A13",
 *          "clone-depth": "100"
 *     }
 * ]
 * Only "repository" and "target_path" are the required keys in each object.
 * If "remote" is not specified then there are two options, the value of "repository" should
 * be like username/device_brand_name such that the repository link can be obtained
 * by simply prefixing https://github.com/, if that is not the case then flamingo-devices
 * remote is used as the default. If "revision" is not specified then the remote must have a
 * default revision set in manifest.
 */
use async_recursion::async_recursion;
use clap::Parser;
use dependency::Dependency;
use json::JsonValue;
use manifest::Manifest;
use regex::Regex;
use remotes::Remote;
use reqwest::{Client, StatusCode};
use std::{
    collections::HashMap,
    fs,
    process::{Command, ExitStatus},
};

mod dependency;
mod manifest;
mod remotes;

const ORG: &str = "FlamingoOS-Devices";
const DEFAULT_BRANCH: &str = "A13";
const DEPENDENCY_FILE_NAME: &str = "flamingo.dependencies";

const LOCAL_MANIFESTS_DIR: &str = "local_manifests";
const SOURCE_MANIFESTS_DIR: &str = "manifests";

const RESPONSE_KEY_NAME: &str = "name";

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    manifest_root: String,

    #[arg(short, long)]
    device_name: String,

    #[arg(short, long, default_value_t = DEFAULT_BRANCH.to_owned())]
    branch: String,

    #[arg(short, long, default_value_t = false)]
    sync: bool,

    #[arg(short, long, default_value_t = false)]
    quiet: bool,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let args = Args::parse();

    let client = Client::new();
    let repo_pattern = format!(r"device_.*_{}", &args.device_name);
    let repo_regex = Regex::new(&repo_pattern).unwrap();

    if !args.quiet {
        println!("Searching for {} repository in {ORG}", &args.device_name);
    }
    let device_repo = find_device_repo(&client, &repo_regex, 1).await?;
    if !args.quiet {
        println!("Found device repository {device_repo}");
    }

    let remotes =
        remotes::get_all_remotes(&format!("{}/{SOURCE_MANIFESTS_DIR}", args.manifest_root))?;

    let local_manifest_dir = format!("{}/{LOCAL_MANIFESTS_DIR}", args.manifest_root);
    fs::create_dir_all(&local_manifest_dir)
        .map_err(|err| format!("failed to create local manifest dir: {err}"))?;

    let device_dependency = Dependency {
        name: format!("{ORG}/{device_repo}"),
        path: device_repo.replace("_", "/"),
        remote: remotes::FLAMINGO_DEVICES.to_owned(),
        branch: args.branch.to_owned(),
        clone_depth: None,
    };
    let all_dependencies = get_dependencies(
        &client,
        &local_manifest_dir,
        &device_dependency,
        &remotes,
        args.quiet,
    )
    .await?;
    let dependencies = create_manifest(device_dependency, all_dependencies, &local_manifest_dir)?;
    if args.sync {
        let status = sync_dependencies(&dependencies)?;
        println!("child process exited with status: {}", status.to_string());
    } else {
        println!("Projects are:");
        dependencies.iter().for_each(|dep| println!("{}", dep.path));
    }
    Ok(())
}

/// Attempts to get the name of the repo for the device name.
/// The results from github api is paginated, therefore this
/// function is recusively called until the all results are
/// covered or a repo with matching pattern is found.
#[async_recursion]
async fn find_device_repo(client: &Client, regex: &Regex, page: u32) -> Result<String, String> {
    let response = client
        .get(format!("https://api.github.com/orgs/{ORG}/repos"))
        .header("accept", "application/vnd.github+json")
        .header("User-Agent", ORG)
        .query(&[
            ("type", "public"),
            ("per_page", "100"),
            ("page", &page.to_string()),
        ])
        .send()
        .await
        .map_err(|err| format!("GET request to list repositories failed: {err}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "GET request to list repositories failed. Status code = {}",
            response.status().as_str()
        ));
    }
    let json_response = response
        .text()
        .await
        .map_err(|err| format!("Failed to get json response: {err}"))?;
    let json = json::parse(&json_response).map_err(|err| format!("Failed to parse json: {err}"))?;
    match json {
        JsonValue::Array(repos) => {
            if repos.is_empty() {
                return Err(String::from("Failed to find repository"));
            }
            let repo_name = repos
                .iter()
                .filter_map(|value| {
                    if let JsonValue::Object(object) = value {
                        object
                            .get(RESPONSE_KEY_NAME)
                            .map(|value| value.as_str())
                            .flatten()
                    } else {
                        None
                    }
                })
                .find(|name| regex.is_match(name));
            if repo_name.is_none() {
                find_device_repo(client, regex, page + 1).await
            } else {
                Ok(repo_name.unwrap().to_owned())
            }
        }
        other => Err(format!(
            "GET response returned unexpected json response: {}",
            other.pretty(4)
        )),
    }
}

fn get_deps_url(repo_name: &str, branch: &str) -> String {
    format!("https://raw.githubusercontent.com/{repo_name}/{branch}/{DEPENDENCY_FILE_NAME}")
}

/// This is where the magic happens. The starting point will
/// be device repo, dependecies in it will be fetched, and then
/// recursively checks for their dependencies as well.
#[async_recursion]
async fn get_dependencies(
    client: &Client,
    local_manifest_dir: &str,
    dependency: &Dependency,
    remotes: &HashMap<String, Remote>,
    quiet: bool,
) -> Result<Vec<Dependency>, String> {
    if !quiet {
        println!("Looking for dependencies in {}", dependency.name);
    }

    let deps_url = get_deps_url(&dependency.name, &dependency.branch);
    let response = client
        .get(&deps_url)
        .send()
        .await
        .map_err(|err| format!("Failed to get dependency file from {deps_url}: {err}"))?;
    if response.status() == StatusCode::NOT_FOUND {
        if !quiet {
            println!("No dependencies in {}", dependency.name);
        }
        return Ok(Vec::with_capacity(0));
    }
    if !response.status().is_success() {
        return Err(format!(
            "GET request to {deps_url} failed. Status code = {}",
            response.status().as_str()
        ));
    }
    let json_response = response
        .text()
        .await
        .map_err(|err| format!("Failed to get dependency file as json: {err}"))?;
    let deps = json::parse(&json_response).map_err(|err| format!("Failed to parse json: {err}"))?;
    match deps {
        JsonValue::Array(repos) => {
            let mut dependencies = Vec::new();
            for repo in repos {
                let sub_dependency = Dependency::get(repo, remotes)?;
                let sub_dependencies =
                    get_dependencies(client, local_manifest_dir, &sub_dependency, remotes, quiet)
                        .await?;
                dependencies.push(sub_dependency);
                dependencies.extend(sub_dependencies);
            }
            Ok(dependencies)
        }
        other => Err(format!("Unexpected element {other} in dependency json")),
    }
}

fn create_manifest(
    device_dependency: Dependency,
    all_dependencies: Vec<Dependency>,
    local_manifest_dir: &str,
) -> Result<Vec<Dependency>, String> {
    let mut dependencies = Vec::with_capacity(all_dependencies.len() + 1);
    dependencies.push(device_dependency);
    dependencies.extend(all_dependencies);
    let mut manifest = Manifest::new();
    manifest.add_dependencies(&dependencies);
    manifest.write(&local_manifest_dir)?;
    Ok(dependencies)
}

fn sync_dependencies(dependencies: &Vec<Dependency>) -> Result<ExitStatus, String> {
    let sync_args = [
        "--force-sync",
        "--no-tags",
        "--current-branch",
        "--no-clone-bundle",
    ];
    let mut child = Command::new("repo")
        .arg("sync")
        .args(sync_args)
        .args(
            dependencies
                .iter()
                .map(|dependency| dependency.path.as_str()),
        )
        .spawn()
        .map_err(|err| format!("failed to spawn repo sync process: {err}"))?;
    child
        .wait()
        .map_err(|err| format!("failed to wait on child process: {err}"))
}
