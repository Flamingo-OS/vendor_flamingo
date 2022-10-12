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

use git2::{Cred, Error, ErrorCode, PushOptions, Remote, RemoteCallbacks, Repository};
use std::collections::HashMap;
use std::fmt::Display;
use std::option::Option;
use threadpool::ThreadPool;

use crate::ATTR_NAME;
use crate::ATTR_PATH;
use crate::{
    errors::error,
    manifest::{read_manifest, Manifest, ManifestFmt},
};

const FLAMINGO_REMOTE: &str = "flamingo";
const FLAMINGO_BRANCH: &str = "A13";

#[derive(Clone)]
struct MergeData {
    remote_name: String,
    remote_url: String,
    repo_path: String,
    repo_name: String,
    revision: String,
    push: bool,
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
                let path = element.attributes.get(ATTR_PATH).unwrap().to_string();
                let name = element.attributes.get(ATTR_NAME).unwrap().to_string();
                (path, name)
            })
            .collect()
    })
}

pub fn merge_upstream(
    source: String,
    flamingo_manifest: Manifest,
    system_manifest: &Option<Manifest>,
    vendor_manifest: &Option<Manifest>,
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
            if let Err(err) = fetch(&mut remote, merge_data.clone()) {
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
            match push(&repo, merge_data.repo_name.clone()) {
                Ok(_) => {
                    println!("Successfully pushed {}", &merge_data.repo_name);
                }
                Err(err) => {
                    error(&format!(
                        "failed to push {} : {err}",
                        merge_data.repo_name.clone()
                    ));
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

fn fetch(remote: &mut Remote, merge_data: MergeData) -> Result<(), String> {
    remote
        .fetch(&[merge_data.revision.clone()], None, None)
        .map_err(|err| {
            format!(
                "failed to fetch revision {} from {} : {err}",
                &merge_data.revision, &merge_data.remote_url
            )
        })
}

fn push(repository: &Repository, name: String) -> Result<(), Error> {
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
