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

use crate::dependency::Dependency;
use std::fs::File;
use xmltree::{Element, EmitterConfig, XMLNode};

pub mod defs {
    pub const DEVICE_MANIFEST_FILE_NAME: &str = "device_manifest";
    pub const MANIFEST_EXT: &str = "xml";

    pub const MANIFEST_ELEMENT: &str = "manifest";
    pub const REMOTE_ELEMENT: &str = "remote";
    pub const PROJECT_ELEMENT: &str = "project";

    pub const ATTR_NAME: &str = "name";
    pub const ATTR_PATH: &str = "path";
    pub const ATTR_FETCH: &str = "fetch";
    pub const ATTR_REMOTE: &str = "remote";
    pub const ATTR_REVISION: &str = "revision";
    pub const ATTR_CLONE_DEPTH: &str = "clone-depth";

    pub const INDENT: &str = "    ";
}

pub struct Manifest {
    xml: Element,
}

impl Manifest {
    pub fn new() -> Self {
        Self {
            xml: Element::new(defs::MANIFEST_ELEMENT),
        }
    }

    pub fn add_dependencies(&mut self, dependencies: &Vec<Dependency>) {
        dependencies
            .iter()
            .map(|dependency| {
                let mut project_element = Element::new(defs::PROJECT_ELEMENT);
                let attrs = &mut project_element.attributes;
                attrs.insert(defs::ATTR_NAME.to_owned(), get_project_name(dependency));
                attrs.insert(defs::ATTR_PATH.to_owned(), dependency.path.to_owned());
                attrs.insert(defs::ATTR_REMOTE.to_owned(), dependency.remote.to_owned());
                attrs.insert(defs::ATTR_REVISION.to_owned(), dependency.branch.to_owned());
                if let Some(depth) = dependency.clone_depth.as_ref() {
                    attrs.insert(defs::ATTR_CLONE_DEPTH.to_owned(), depth.to_owned());
                }
                project_element
            })
            .for_each(|element| self.xml.children.push(XMLNode::Element(element)));
    }

    pub fn write(&self, dir: &str) -> Result<(), String> {
        let file = File::create(format!(
            "{dir}/{}.{}",
            defs::DEVICE_MANIFEST_FILE_NAME,
            defs::MANIFEST_EXT
        ))
        .map_err(|err| format!("failed to create manifest file in {dir}: {err}"))?;
        let config = EmitterConfig::new()
            .indent_string(defs::INDENT)
            .perform_indent(true);
        self.xml
            .write_with_config(file, config)
            .map_err(|err| format!("{err}"))
    }
}

fn get_project_name(dependency: &Dependency) -> String {
    if dependency.name.contains("/") {
        dependency.name.rsplit_once('/').unwrap().1.to_owned()
    } else {
        dependency.name.to_owned()
    }
}
