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

use std::collections::HashMap;
use std::fs::{File, OpenOptions};

use git2::Repository;
use reqwest::Client;
use std::collections::HashSet;
use std::io::{BufReader, Read};
use std::option::Option;
use std::vec::Vec;
use xmltree::{Element, EmitterConfig, XMLNode};

use crate::git;

const ELEMENT_MANIFEST: &str = "manifest";
const ELEMENT_PROJECT: &str = "project";

const ATTR_NAME: &str = "name";
const ATTR_PATH: &str = "path";
const ATTR_REMOTE: &str = "remote";
const ATTR_REVISION: &str = "revision";
const ATTR_CLONE_DEPTH: &str = "clone-depth";

const XML_INDENT: &str = "    ";

pub struct Manifest {
    name: String,
    path: String,
    tag: Option<String>,
}

impl Manifest {
    pub fn new(dir: &str, name: &str, tag: Option<String>) -> Self {
        Self {
            name: name.to_owned(),
            path: format!("{dir}/{name}.xml"),
            tag,
        }
    }

    pub fn get_name(&self) -> String {
        format!("{}.xml", self.name)
    }

    pub fn get_url(&self) -> Option<String> {
        self.tag.as_ref().map(|tag| {
            format!(
                "https://git.codelinaro.org/clo/la/la/{0}/manifest/-/raw/{1}/{1}.xml",
                self.name, tag
            )
        })
    }

    pub fn get_remote_name(&self) -> String {
        format!("clo_{}", self.name)
    }

    pub fn get_remote_url(&self) -> String {
        String::from("https://git.codelinaro.org/clo/la")
    }

    pub fn get_aosp_remote_url(&self) -> String {
        format!("https://android.googlesource.com/platform")
    }

    pub fn get_revision(&self) -> Option<String> {
        self.tag.as_ref().map(|tag| format!("refs/tags/{tag}"))
    }

    pub fn get_repo_path(&self) -> String {
        let splt_path = self
            .path
            .split("/")
            .map(|s| s.to_owned())
            .collect::<Vec<String>>();
        splt_path[..splt_path.len() - 1].join("/")
    }

    pub fn get_truncated_file(&self) -> Result<File, String> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(true)
            .open(&self.path)
            .map_err(|err| format!("Failed to create {}: {err}", self.get_name()))
    }

    pub fn get_file(&self) -> Result<File, String> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.path)
            .map_err(|err| format!("Failed to create {}: {err}", self.get_name()))
    }
}

pub async fn update(client: &Client, manifest: &Option<Manifest>) -> Result<(), String> {
    let manifest = match manifest {
        Some(manifest) => manifest,
        None => return Ok(()),
    };
    let xml_manifest = download_manifest(&client, manifest)
        .await
        .map_err(|err| format!("Failed to get manifest: {}", err))?;
    let config = EmitterConfig::new()
        .indent_string(XML_INDENT)
        .perform_indent(true);
    let file = manifest.get_truncated_file()?;
    xml_manifest
        .write_with_config(file, config)
        .map_err(|err| format!("failed to write manifest: {}", err))
}

async fn download_manifest(client: &Client, manifest: &Manifest) -> Result<Element, String> {
    let url = manifest.get_url().ok_or(format!(
        "Manifest {} does not contain a valid tag",
        manifest.name
    ))?;
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|err| format!("Error while sending GET request: {err}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "GET request to {url} failed. Status code = {}",
            response.status().as_str()
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("Failed to get response body: {err}"))?;
    let xml_manifest =
        Element::parse(&bytes[..]).map_err(|err| format!("Failed to parse manifest: {err}"))?;
    Ok(transform_manifest(
        xml_manifest,
        &manifest.get_remote_name(),
    ))
}

fn transform_manifest(manifest: Element, remote: &String) -> Element {
    // Filter child elements of <manifest></manifest>
    // Currently we only care about <project> elements.
    let elements_to_keep = HashSet::from([ELEMENT_PROJECT.to_owned()]);

    // Remove attributes from <project> elements.
    let attrs_to_keep = HashSet::from([
        ATTR_CLONE_DEPTH.to_owned(),
        ATTR_NAME.to_owned(),
        ATTR_PATH.to_owned(),
    ]);

    // Shallow clone (clone-depth="1") some big repos by default
    // to save space in machine.
    let shallow_clone_repos = HashSet::from([
        String::from("platform/external/"),
        String::from("platform/prebuilts/"),
    ]);

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
        .for_each(|node| {
            let node = if let XMLNode::Element(elem) = node {
                let mut filtered_element = Element {
                    attributes: elem
                        .attributes
                        .iter()
                        .filter(|(key, _)| attrs_to_keep.contains(*key))
                        .map(|(key, value)| (key.to_owned(), value.to_owned()))
                        .collect(),
                    ..elem.to_owned()
                };

                let attrs = &mut filtered_element.attributes;

                // Some repos have clone-depth="2", let's just keep
                // it 1 for our sake.
                attrs
                    .entry(ATTR_CLONE_DEPTH.to_string())
                    .and_modify(|depth| *depth = String::from("1"));

                // Set remote from our default.xml manifest
                attrs.insert(ATTR_REMOTE.to_string(), remote.to_owned());

                let name = attrs.get(ATTR_NAME).unwrap();
                let should_shallow_clone = shallow_clone_repos
                    .iter()
                    .any(|prefix| name.starts_with(prefix));
                if should_shallow_clone {
                    attrs
                        .entry(ATTR_CLONE_DEPTH.to_string())
                        .or_insert(String::from("1"));
                }
                XMLNode::Element(filtered_element)
            } else {
                node.to_owned()
            };
            transformed_manifest.children.push(node)
        });
    transformed_manifest
}

fn read_manifest(manifest: &Manifest) -> Result<Element, String> {
    let mut bytes: Vec<u8> = Vec::new();
    let file = manifest.get_file()?;
    let mut reader = BufReader::new(file);
    let bytes_read = reader
        .read_to_end(&mut bytes)
        .map_err(|err| format!("Failed to read {}: {err}", manifest.get_name()))?;
    Element::parse(&bytes[..bytes_read])
        .map_err(|err| format!("Failed to parse {}: {err}", manifest.get_name()))
}

pub fn get_repos(manifest: &Manifest) -> Result<HashMap<String, String>, String> {
    read_manifest(manifest).map(|manifest| {
        manifest
            .children
            .iter()
            .filter_map(|node| node.as_element())
            .filter_map(|element| {
                let attrs = &element.attributes;
                let mapper = |attr: &String| attr.to_owned();
                attrs
                    .get(ATTR_PATH)
                    .map(mapper)
                    .zip(attrs.get(ATTR_NAME).map(mapper))
            })
            .collect()
    })
}

pub fn update_default(
    default_manifest: Manifest,
    system_manifest: &Option<Manifest>,
    vendor_manifest: &Option<Manifest>,
    push: bool
) -> Result<(), String> {
    let mut xml_manifest = read_manifest(&default_manifest)
        .map_err(|err| format!("Failed to parse {}: {err}", default_manifest.get_name()))?;
    xml_manifest
        .children
        .iter_mut()
        .filter_map(|node| node.as_mut_element())
        .filter(|element| element.name == ATTR_REMOTE)
        .map(|element| &mut element.attributes)
        .for_each(|attrs| {
            let remote_name = attrs.get(ATTR_NAME).map(|name_str| name_str.to_owned());
            if remote_name == None {
                error!(
                    "Remote element attributes {:?} does not have key {ATTR_NAME}",
                    attrs
                );
                return;
            }
            let remote_name = remote_name.unwrap();
            attrs
                .entry(ATTR_REVISION.to_owned())
                .and_modify(|revision| {
                    if system_manifest.is_some() {
                        let system_manifest = system_manifest.as_ref().unwrap();
                        if remote_name == system_manifest.get_remote_name() {
                            let system_revision = system_manifest.get_revision();
                            if system_revision.is_some() {
                                *revision = system_revision.unwrap();
                            }
                        }
                    } else if vendor_manifest.is_some() {
                        let vendor_manifest = vendor_manifest.as_ref().unwrap();
                        if remote_name == vendor_manifest.get_remote_name() {
                            let vendor_revision = vendor_manifest.get_revision();
                            if vendor_revision.is_some() {
                                *revision = vendor_revision.unwrap();
                            }
                        }
                    }
                });
        });
    let file = default_manifest.get_truncated_file()?;
    let config = EmitterConfig::new()
        .indent_string(XML_INDENT)
        .perform_indent(true);
    xml_manifest
        .write_with_config(file, config)
        .map_err(|err| format!("failed to write manifest: {}", err))?;
    let repo = Repository::open(default_manifest.get_repo_path())
        .map_err(|err| format!("Failed to open manifest repository: {err}"))?;
    if system_manifest.as_ref().is_some() {
        let msg = format!(
            "system: Update default manifest to {}",
            system_manifest.as_ref().unwrap().get_revision().unwrap()
        );
        println!("Committing: {}", msg);
        git::add_and_commit(&repo, "*", &msg)
            .map_err(|err| format!("Failed to commit version change: {err}"))?;
    } else {
        let msg = format!(
            "vendor: Update default manifest to {}",
            vendor_manifest.as_ref().unwrap().get_revision().unwrap()
        );
        git::add_and_commit(&repo, "*", &msg)
            .map_err(|err| format!("Failed to commit version change: {err}"))?;
    }
    if push {
        git::push(&repo).map_err(|err| format!("Failed to push manifest repo: {err}"))
    } else {
        Ok(())
    }
}
