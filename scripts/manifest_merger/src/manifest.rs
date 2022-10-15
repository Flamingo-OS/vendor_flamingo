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

use reqwest::Client;
use std::collections::HashSet;
use std::io::{BufReader, Read};
use std::option::Option;
use std::vec::Vec;
use xmltree::{Element, EmitterConfig, XMLNode};

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
            name: name.to_string(),
            path: format!("{dir}/{name}.xml"),
            tag,
        }
    }

    pub fn get_name(&self) -> String {
        format!("{}.xml", self.name)
    }

    pub fn get_url(&self) -> String {
        format!(
            "https://git.codelinaro.org/clo/la/la/{0}/manifest/-/raw/{1}/{1}.xml",
            self.name,
            self.tag.as_ref().unwrap_or(&String::new())
        )
    }

    pub fn get_remote_name(&self) -> String {
        format!("clo_{}", self.name)
    }

    pub fn get_remote_url(&self) -> String {
        String::from("https://git.codelinaro.org/clo/la")
    }

    pub fn get_revision(&self) -> String {
        format!("refs/tags/{}", self.tag.as_ref().unwrap_or(&String::new()))
    }

    pub fn get_file(&self) -> File {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.path)
            .expect(&format!("Failed to create {}.xml manifest file", self.name))
    }
}

pub async fn update(client: &Client, manifest: &Option<Manifest>) {
    let manifest = match manifest {
        Some(manifest) => manifest,
        None => return,
    };
    let result = download_manifest(&client, manifest).await;
    match result {
        Ok(xml_manifest) => {
            let config = EmitterConfig::new()
                .indent_string(XML_INDENT)
                .perform_indent(true);
            if let Err(err) = xml_manifest.write_with_config(manifest.get_file(), config) {
                error_exit!("failed to write manifest: {}", err);
            }
        }
        Err(err) => {
            error_exit!("failed to get manifest: {}", err);
        }
    }
}

async fn download_manifest(
    client: &Client,
    manifest: &Manifest,
) -> Result<Element, reqwest::Error> {
    let response = client.get(manifest.get_url()).send().await?;
    if !response.status().is_success() {
        error_exit!(
            "GET request to {0} failed. Status code = {1}",
            manifest.get_url(),
            response.status().as_str()
        );
    }
    let bytes = response.bytes().await.expect("Failed to get response body");
    let xml_manifest = Element::parse(&bytes[..]).expect("Failed to parse manifest");
    Ok(transform_manifest(
        xml_manifest,
        &manifest.get_remote_name(),
    ))
}

fn transform_manifest(manifest: Element, remote: &String) -> Element {
    // Filter child elements of <manifest></manifest>
    // Currently we only care about <project> elements.
    let elements_to_keep = HashSet::from([ELEMENT_PROJECT.to_string()]);

    // Remove attributes from <project> elements.
    let attrs_to_keep = HashSet::from([
        ATTR_CLONE_DEPTH.to_string(),
        ATTR_NAME.to_string(),
        ATTR_PATH.to_string(),
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
                        .entry(ATTR_CLONE_DEPTH.to_string())
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

fn read_manifest(manifest: &Manifest) -> Result<Element, String> {
    let mut bytes: Vec<u8> = Vec::new();
    let mut reader = BufReader::new(manifest.get_file());
    let read_result = reader.read_to_end(&mut bytes);
    match read_result {
        Ok(bytes_read) => {
            let parse_result = Element::parse(&bytes[..bytes_read]);
            match parse_result {
                Ok(element) => Ok(element),
                Err(err) => Err(format!("Failed to parse {}: {err}", manifest.get_name())),
            }
        }
        Err(err) => Err(format!("Failed to read {}: {err}", manifest.get_name())),
    }
}

pub fn get_repos(manifest: &Manifest) -> Result<HashMap<String, String>, String> {
    read_manifest(manifest).map(|manifest| {
        manifest
            .children
            .iter()
            .map(|node| node.as_element())
            .filter(|element| element.is_some())
            .map(|element| element.unwrap())
            .filter(|element| {
                element.attributes.contains_key(ATTR_PATH)
                    && element.attributes.contains_key(ATTR_NAME)
            })
            .map(|element| {
                let path = element.attributes.get(ATTR_PATH).unwrap().to_owned();
                let name = element.attributes.get(ATTR_NAME).unwrap().to_owned();
                (path, name)
            })
            .collect()
    })
}

pub fn update_default(
    default_manifest: Manifest,
    system_manifest: &Option<Manifest>,
    vendor_manifest: &Option<Manifest>,
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
                    .to_string();
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
