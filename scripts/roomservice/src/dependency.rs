use crate::remotes::{self, Remote};
use json::{object::Object, JsonValue};
use std::collections::HashMap;

const DEPS_KEY_NAME: &str = "repository";
const DEPS_KEY_PATH: &str = "target_path";
const DEPS_KEY_REMOTE: &str = "remote";
const DEPS_KEY_BRANCH: &str = "branch";
const DEPS_KEY_DEPTH: &str = "clone-depth";

#[derive(Clone, Debug)]
pub struct Dependency {
    pub name: String,
    pub path: String,
    pub remote: String,
    pub branch: String,
    pub clone_depth: Option<String>,
}

impl Dependency {
    pub fn get(json: JsonValue, remotes: &HashMap<String, Remote>) -> Result<Dependency, String> {
        if let JsonValue::Object(repo) = json {
            let name = get_string(&repo, DEPS_KEY_NAME).ok_or(format!(
                "Dependency {} does not contain string value for key {DEPS_KEY_NAME}",
                repo.pretty(4)
            ))?;
            let path = get_string(&repo, DEPS_KEY_PATH).ok_or(format!(
                "Dependency {} does not contain string value for key {DEPS_KEY_PATH}",
                repo.pretty(4)
            ))?;
            let remote = get_string(&repo, DEPS_KEY_REMOTE).unwrap_or(
                if name.contains("/") {
                    remotes::GITHUB
                } else {
                    remotes::FLAMINGO_DEVICES
                }
                .to_owned(),
            );
            let repo_name = match remote.as_str() {
                remotes::GITHUB => Ok::<String, String>(name.to_owned()),
                other => {
                    // remote.fetch will be like (ex) https://github.com/Flamingo-OS, we need to prefix
                    // Flamingo-OS with the name in this case to pass into get_deps_url.
                    let remote = remotes
                        .get(other)
                        .ok_or(format!("No such remote exists with the name {other}"))?;
                    let prefix = remote
                        .fetch
                        .trim_end_matches('/')
                        .rsplit_once('/')
                        .ok_or(format!("Remote {:?} is not well defined", remote))?;
                    Ok(format!("{}/{name}", prefix.1))
                }
            }?;
            let branch = match get_string(&repo, DEPS_KEY_BRANCH) {
                Some(revision) => Ok::<String, String>(revision),
                None => {
                    match remote.as_str() {
                        remotes::GITHUB => Err(String::from("nigga")),
                        other => {
                            // At this point remote exists and well defined hence using direct access.
                            let remote = &remotes[other];
                            remote
                                .revision
                                .as_ref()
                                .map(|rev| rev.to_owned())
                                .ok_or(format!("Remote {other} does not have a default revision"))
                        }
                    }
                }
            }?;
            let clone_depth = get_string(&repo, DEPS_KEY_DEPTH);
            Ok(Dependency {
                name: repo_name,
                path: path,
                remote: remote,
                branch: branch,
                clone_depth: clone_depth,
            })
        } else {
            return Err(format!("{json} is not an Object"));
        }
    }
}

fn get_string(object: &Object, key: &str) -> Option<String> {
    object
        .get(key)
        .filter(|value| value.is_string())
        .map(|value| match value {
            JsonValue::String(string) => string.to_owned(),
            JsonValue::Short(short) => short.to_string(),
            other => panic!("{} is not a string", other),
        })
}
