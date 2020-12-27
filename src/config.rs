/* ┌────────────────────────────────────────────────────────────────────────────────────────────┐ *\
 * │                                                                                            │ *
 * │ This Source Code Form is subject to the terms of the Mozilla Public                        │ *
 * │ License, v. 2.0. If a copy of the MPL was not distributed with this                        │ *
 * │ file, You can obtain one at http://mozilla.org/MPL/2.0/.                                   │ *
 * │                                                                                            │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

/* ┌────────────────────────────────────────────────────────────────────────────────────────────┐ *\
 * │                                            use                                             │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

use super::FileSystem;
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;

/* ┌────────────────────────────────────────────────────────────────────────────────────────────┐ *\
 * │                                           Config                                           │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

#[derive(Deserialize)]
pub struct Config {
    projects: Vec<Project>,
    mirrors: Vec<Mirror>,
}

impl Config {
/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                    load_into()                                     │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    pub fn load_into(self, fs: &mut FileSystem) {
        let mut groups = HashMap::new();

        for project in self.projects {
            std::fs::metadata(&project.path).unwrap();

            let pgroups = project.groups.into_iter().map(|group| {
                if let Some(ino) = groups.get(&group) {
                    *ino
                } else {
                    let ino = fs.add_group(group.clone());
                    groups.insert(group, ino);
                    ino
                }
            }).collect::<Vec<_>>();

            fs.add_project(
                project.name,
                project.path,
                pgroups.into_iter(),
                &project.aliases,
            );
        }

        for mirror in self.mirrors {
            std::fs::metadata(&mirror.path).unwrap();

            fs.add_mirror(
                mirror.name,
                mirror.path,
                Regex::new(&mirror.renaming).unwrap(),
                &mirror.aliases,
            );
        }
    }
}

/* ┌────────────────────────────────────────────────────────────────────────────────────────────┐ *\
 * │                                          Project                                           │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

#[derive(Deserialize)]
struct Project {
    name: String,
    path: String,
    groups: Vec<String>,
    aliases: Vec<String>,
}

/* ┌────────────────────────────────────────────────────────────────────────────────────────────┐ *\
 * │                                           Mirror                                           │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

#[derive(Deserialize)]
struct Mirror {
    name: String,
    path: String,
    renaming: String,
    aliases: Vec<String>,
}
