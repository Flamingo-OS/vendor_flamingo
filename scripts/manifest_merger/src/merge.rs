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

use crate::{
    git,
    manifest::{self, Manifest},
};
use git2::Repository;
use std::collections::HashMap;
use std::option::Option;
use threadpool::ThreadPool;

struct MergeData {
    remote_name: String,
    remote_url: String,
    repo_path: String,
    repo_name: String,
    revision: String,
    push: bool,
}

pub fn merge_upstream(
    source: &str,
    flamingo_manifest: Manifest,
    system_manifest: &Option<Manifest>,
    vendor_manifest: &Option<Manifest>,
    thread_count: usize,
    push: bool,
) {
    let flamingo_repos = manifest::get_repos(&flamingo_manifest).unwrap();
    let system_repos = system_manifest.as_ref().map_or(HashMap::new(), |manifest| {
        manifest::get_repos(manifest).unwrap()
    });
    let vendor_repos = vendor_manifest.as_ref().map_or(HashMap::new(), |manifest| {
        manifest::get_repos(manifest).unwrap()
    });

    let thread_pool = ThreadPool::new(thread_count);
    flamingo_repos
        .iter()
        .map(|(path, _)| {
            if system_manifest.is_some() && system_repos.contains_key(&path[..]) {
                let system_manifest = system_manifest.as_ref().unwrap();
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
                let vendor_manifest = vendor_manifest.as_ref().unwrap();
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
            let result =
                git::get_or_create_remote(&repo, &merge_data.remote_name, &merge_data.remote_url);
            if let Err(err) = result {
                error_exit!("{}", err);
            }
            if let Err(err) = result.unwrap().fetch(&[&merge_data.revision], None, None) {
                error!(
                    "failed to fetch revision {} from {} : {err}",
                    &merge_data.revision, &merge_data.remote_url
                );
                return;
            }
            let reference = repo.find_reference(&merge_data.revision).unwrap();
            let annotated_commit = repo.reference_to_annotated_commit(&reference).unwrap();
            if let Err(err) = repo.merge(&[&annotated_commit], None, None) {
                error!(
                    "failed to merge revision {} from {} in {} : {err}",
                    &merge_data.revision, &merge_data.remote_url, &merge_data.repo_name
                );
                return;
            }
            if !merge_data.push {
                return;
            }
            match git::push(&repo, &merge_data.repo_name) {
                Ok(_) => println!("Successfully pushed {}", &merge_data.repo_name),
                Err(err) => error!("failed to push {} : {err}", &merge_data.repo_name),
            }
        }
        Err(err) => {
            error!(
                "failed to open git repository at {}: {err}",
                &merge_data.repo_path
            );
        }
    }
}
