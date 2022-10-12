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

use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::fs::File;
use std::fs::OpenOptions;

pub trait ManifestFmt {
    fn get_file(&self) -> File;
}

pub struct Manifest {
    name: String,
    path: String,
}

impl Manifest {
    fn create(dir: &str, name: &str) -> Self {
        Self {
            name: format!("{name}.xml"),
            path: format!("{dir}/{name}.xml"),
        }
    }

    pub fn default(dir: &str) -> Self {
        Self::create(dir, "default")
    }

    pub fn flamingo(dir: &str) -> Self {
        Self::create(dir, "flamingo")
    }
}

impl Display for Manifest {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.name)
    }
}

impl ManifestFmt for Manifest {
    fn get_file(&self) -> File {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.path)
            .expect(&format!("Failed to create {} manifest file", self.name))
    }
}

pub struct CloManifest {
    name: String,
    tag: String,
    path: String,
}

impl CloManifest {
    fn create(dir: &str, tag: String, name: &str) -> Self {
        Self {
            name: name.to_owned(),
            tag: tag,
            path: format!("{dir}/{name}.xml"),
        }
    }

    pub fn system(dir: &str, tag: String) -> Self {
        Self::create(dir, tag, "system")
    }

    pub fn vendor(dir: &str, tag: String) -> Self {
        Self::create(dir, tag, "vendor")
    }

    pub fn get_url(&self) -> String {
        format!(
            "https://git.codelinaro.org/clo/la/la/{0}/manifest/-/raw/{1}/{1}.xml",
            self.name, self.tag
        )
    }

    pub fn get_remote_name(&self) -> String {
        format!("clo_{}", self.name)
    }

    pub fn get_remote_url(&self) -> String {
        String::from("https://git.codelinaro.org/clo/la")
    }

    pub fn get_revision(&self) -> String {
        format!("refs/tags/{}", self.tag)
    }
}

impl Display for CloManifest {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}.xml", self.name)
    }
}

impl ManifestFmt for CloManifest {
    fn get_file(&self) -> File {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&self.path)
            .expect(&format!("Failed to create {}.xml manifest file", self.name))
    }
}
