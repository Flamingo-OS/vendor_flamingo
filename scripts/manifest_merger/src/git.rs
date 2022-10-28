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

use git2::{
    Cred, Error, ErrorCode, IndexAddOption, Oid, PushOptions, Remote, RemoteCallbacks, Repository,
};
use std::path::Path;

const FLAMINGO_REMOTE: &str = "flamingo";
const FLAMINGO_BRANCH: &str = "A13";

pub fn get_or_create_remote<'a>(
    repo: &'a Repository,
    name: &'a str,
    url: &'a str,
) -> Result<Remote<'a>, String> {
    match repo.remote(name, url) {
        Ok(remote) => Ok(remote),
        Err(err) => {
            if err.code() == ErrorCode::Exists {
                Ok(repo.find_remote(name).unwrap())
            } else {
                Err(format!("failed to create remote {}: {err}", name))
            }
        }
    }
}

pub fn try_create_remote<'a>(
    repo: &'a Repository,
    name: &'a str,
    url: &'a str,
) -> Result<(), String> {
    match repo.remote(name, url) {
        Ok(_) => Ok(()),
        Err(err) => {
            if err.code() == ErrorCode::Exists {
                Ok(())
            } else {
                Err(format!("failed to create remote {}: {err}", name))
            }
        }
    }
}

pub fn add_and_commit(
    repository: &Repository,
    pathspec: &str,
    message: &str,
) -> Result<Oid, Error> {
    let mut index = repository.index()?;
    let mut options = IndexAddOption::empty();
    options.insert(IndexAddOption::DEFAULT);
    index.add_all(&[pathspec], options, None)?;
    let oid = index.write_tree()?;
    index.write()?;
    let signature = repository.signature()?;
    let parent_commit = repository.head()?.peel_to_commit()?;
    let tree = repository.find_tree(oid)?;
    repository.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &message,
        &tree,
        &[&parent_commit],
    )
}

fn get_repo_name(repository: &Repository) -> &str {
    repository
        .path()
        .parent()
        .unwrap()
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
}

pub fn push(repository: &Repository) -> Result<(), Error> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_, username_from_url, _| {
        Cred::ssh_key_from_agent(&username_from_url.unwrap())
    });
    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);
    repository
        .find_remote(FLAMINGO_REMOTE)
        .expect(&format!(
            "Flamingo remote not found in {}",
            get_repo_name(repository)
        ))
        .push(
            &[format!("HEAD:refs/heads/{FLAMINGO_BRANCH}")],
            Some(&mut push_options),
        )
}
