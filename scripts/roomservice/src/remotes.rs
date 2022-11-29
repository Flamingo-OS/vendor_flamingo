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

use crate::manifest::defs;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::Path;
use std::vec::Vec;
use xmltree::Element;

pub const GITHUB: &str = "github";
pub const FLAMINGO_DEVICES: &str = "flamingo-devices";

#[derive(Clone, Debug)]
pub struct Remote {
    pub name: String,
    pub fetch: String,
    pub revision: Option<String>,
}

fn walk_manifest_dir(dir: &Path) -> Result<Vec<String>, String> {
    let mut manifests = Vec::new();
    if dir.is_file() {
        return Ok(manifests);
    }
    let entries =
        fs::read_dir(dir).map_err(|err| format!("Failed to read dir {:?}: {err}", dir))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("Failed to open DirEntry: {err}"))?;
        let path = entry.path();
        if path.is_dir() {
            let sub_tree_manifests = walk_manifest_dir(&path)?;
            manifests.extend(sub_tree_manifests);
        } else {
            let is_xml = path.extension().filter(|ext| *ext == defs::MANIFEST_EXT);
            if is_xml.is_none() {
                continue;
            }
            let path = path.to_str().ok_or(format!(
                "Failed to get absolute path of manifest {:?}",
                path
            ))?;
            manifests.push(path.to_owned());
        }
    }
    return Ok(manifests);
}

fn get_remotes(manifest: &str) -> Result<Vec<Remote>, String> {
    let manifest_file = File::open(manifest)
        .map_err(|err| format!("Failed to open manifest file {manifest}: {err}"))?;
    let mut bytes: Vec<u8> = Vec::new();
    let mut reader = BufReader::new(manifest_file);
    let bytes_read = reader
        .read_to_end(&mut bytes)
        .map_err(|err| format!("Failed to read {manifest}: {err}"))?;
    let xml_element = Element::parse(&bytes[..bytes_read])
        .map_err(|err| format!("Failed to parse {manifest}: {err}"))?;
    let remotes = xml_element
        .children
        .iter()
        .filter_map(|node| node.as_element())
        .filter(|element| element.name == defs::REMOTE_ELEMENT)
        .map(|remote_element| Remote {
            name: remote_element.attributes[defs::ATTR_NAME].to_owned(),
            fetch: remote_element.attributes[defs::ATTR_FETCH].to_owned(),
            revision: remote_element
                .attributes
                .get(defs::ATTR_REVISION)
                .map(|rev| rev.to_owned()),
        })
        .collect();
    Ok(remotes)
}

pub fn get_all_remotes(manifest_dir: &str) -> Result<HashMap<String, Remote>, String> {
    let manifests = walk_manifest_dir(&Path::new(manifest_dir))?;
    let mut all_remotes: HashMap<String, Remote> = HashMap::new();
    for manifest in manifests {
        let remotes = get_remotes(&manifest)?;
        all_remotes.extend(
            remotes
                .iter()
                .map(|remote| (remote.name.to_owned(), remote.clone())),
        );
    }
    return Ok(all_remotes);
}
