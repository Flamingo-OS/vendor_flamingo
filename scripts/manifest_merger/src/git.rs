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

pub fn push(repository: &Repository, name: &str) -> Result<(), Error> {
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
