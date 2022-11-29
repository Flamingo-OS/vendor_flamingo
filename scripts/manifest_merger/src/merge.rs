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
use git2::{
    build::CheckoutBuilder, Error, IndexAddOption, MergeOptions, Repository, StatusOptions,
};
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
) -> Result<(), String> {
    let flamingo_repos = manifest::get_repos(&flamingo_manifest)?;
    let system_repos = system_manifest
        .as_ref()
        .map_or(Ok(HashMap::with_capacity(0)), |manifest| {
            manifest::get_repos(manifest)
        })?;
    let vendor_repos = vendor_manifest
        .as_ref()
        .map_or(Ok(HashMap::with_capacity(0)), |manifest| {
            manifest::get_repos(manifest)
        })?;

    let thread_pool = ThreadPool::new(thread_count);
    flamingo_repos
        .iter()
        .filter_map(|(path, _)| {
            if system_manifest.is_some() && system_repos.contains_key(path) {
                let system_manifest = system_manifest.as_ref().unwrap();
                Some(MergeData {
                    remote_name: system_manifest.get_remote_name(),
                    remote_url: format!(
                        "{}/{}",
                        system_manifest.get_remote_url(),
                        system_repos[path]
                    ),
                    repo_path: format!("{}/{}", source, path),
                    repo_name: path.to_owned(),
                    revision: system_manifest.get_revision().unwrap(),
                    push,
                })
            } else if vendor_manifest.is_some() && vendor_repos.contains_key(path) {
                let vendor_manifest = vendor_manifest.as_ref().unwrap();
                Some(MergeData {
                    remote_name: vendor_manifest.get_remote_name(),
                    remote_url: format!(
                        "{}/{}",
                        vendor_manifest.get_remote_url(),
                        vendor_repos[path]
                    ),
                    repo_path: format!("{}/{}", source, path),
                    repo_name: path.to_owned(),
                    revision: vendor_manifest.get_revision().unwrap(),
                    push,
                })
            } else {
                None
            }
        })
        .for_each(|merge_data| {
            thread_pool.execute(|| {
                let repo_name = merge_data.repo_name.to_owned();
                if let Err(err) = merge_in_repo(merge_data) {
                    error!("failed to merge in {repo_name}: {err}");
                }
            })
        });
    thread_pool.join();
    Ok(())
}

fn merge_in_repo(merge_data: MergeData) -> Result<(), Error> {
    println!("Merging in {}", &merge_data.repo_name);
    let repo = Repository::open(&merge_data.repo_path)?;
    let mut remote =
        git::get_or_create_remote(&repo, &merge_data.remote_name, &merge_data.remote_url)?;
    remote.fetch(&[&merge_data.revision], None, None)?;
    let reference = repo.find_reference(&merge_data.revision)?;
    let annotated_commit = repo.reference_to_annotated_commit(&reference)?;
    repo.merge(
        &[&annotated_commit],
        Some(&mut MergeOptions::default()),
        Some(&mut CheckoutBuilder::default()),
    )?;
    let mut index = repo.index()?;
    if index.has_conflicts() {
        return Err(Error::from_str(&format!(
            "Repo {} has conflicts",
            &merge_data.repo_name
        )));
    }
    index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
    let oid = index.write_tree()?;
    let statuses = repo.statuses(Some(&mut StatusOptions::default()))?;
    if statuses.is_empty() {
        println!("{} is already up-to-date", &merge_data.repo_name);
        return Ok(());
    }
    let signature = repo.signature()?;
    let parent_commit = repo.head()?.peel_to_commit()?;
    let tree = repo.find_tree(oid)?;
    let (_, tag) = merge_data
        .revision
        .rsplit_once('/')
        .ok_or(Error::from_str(&format!(
            "Malformed revision {}",
            merge_data.revision
        )))?;
    let message = format!("Merge tag '{tag}' of {} into HEAD", remote.url().unwrap());
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &message,
        &tree,
        &[&parent_commit],
    )?;
    repo.cleanup_state()?;
    if merge_data.push {
        git::push(&repo)
    } else {
        Ok(())
    }
}
