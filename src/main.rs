/* ┌────────────────────────────────────────────────────────────────────────────────────────────┐ *\
 * │                                                                                            │ *
 * │ This Source Code Form is subject to the terms of the Mozilla Public                        │ *
 * │ License, v. 2.0. If a copy of the MPL was not distributed with this                        │ *
 * │ file, You can obtain one at http://mozilla.org/MPL/2.0/.                                   │ *
 * │                                                                                            │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

/* ┌────────────────────────────────────────────────────────────────────────────────────────────┐ *\
 * │                                          feature                                           │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

#![feature(option_expect_none)]

/* ┌────────────────────────────────────────────────────────────────────────────────────────────┐ *\
 * │                                            mod                                             │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

mod config;
mod groups;
mod projects;

/* ┌────────────────────────────────────────────────────────────────────────────────────────────┐ *\
 * │                                            use                                             │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

use self::config::Config;
use self::groups::{INO as GROUPS_INO, STR as GROUPS_STR, Group, Groups};
use self::projects::{INO as PROJECTS_INO, STR as PROJECTS_STR, Projects};
use fuser::{
    FileAttr, FileType,
    MountOption,
    ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry,
    Request,
};
use libc::ENOENT;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

/* ┌────────────────────────────────────────────────────────────────────────────────────────────┐ *\
 * │                                           const                                            │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

const TTL: Duration = Duration::from_secs(1);
const ROOT_INO: u64 = 1;

/* ┌────────────────────────────────────────────────────────────────────────────────────────────┐ *\
 * │                                           main()                                           │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

fn main() {
    env_logger::init();

    let mut args = pico_args::Arguments::from_env();

    let projects: PathBuf = args
        .opt_value_from_str(["-p", "--projects"])
        .unwrap()
        .unwrap_or("projects.toml".into());
    let mount: PathBuf = args
        .opt_value_from_str(["-m", "--mount"])
        .unwrap()
        .unwrap_or("/code".into());

    let mut fs = FileSystem::new();
    let config = std::fs::read(projects).unwrap();
    toml::from_slice::<Config>(&config).unwrap().load_into(&mut fs);

    fuser::mount2(fs, mount, &[
        MountOption::RO,
        MountOption::AutoUnmount,
        MountOption::FSName("pr0j3c75".into()),
    ]).unwrap();
}

/* ┌────────────────────────────────────────────────────────────────────────────────────────────┐ *\
 * │                                         FileSystem                                         │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

pub struct FileSystem {
    groups: Groups,
    projects: Projects,
}

impl FileSystem {
/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                       new()                                        │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    fn new() -> Self {
        FileSystem {
            groups: Groups::new(),
            projects: Projects::new(),
        }
    }

/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                    add_group()                                     │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    fn add_group(&mut self, name: String) -> u64 {
        self.groups.add_group(name, HashMap::new())
    }

/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                   add_project()                                    │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    fn add_project(&mut self, name: String, path: String, groups: &[u64], aliases: &[String]) -> u64 {
        let ino = self.projects.add_project(
            name.clone(),
            path,
            groups.iter().copied().collect(),
            aliases.iter().cloned().collect(),
        );

        for group in groups {
            let group = self.get_group(*group).expect("unknown group");
            group.add_project(name.clone(), ino);

            for alias in aliases {
                group.add_alias(alias.clone(), ino);
            }
        }

        ino
    }

/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                    get_group()                                     │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    fn get_group(&mut self, ino: u64) -> Option<&mut Group> {
        self.groups.get_group(ino)
    }
}

impl fuser::Filesystem for FileSystem {
    fn getattr(&mut self, _: &Request, ino: u64, reply: ReplyAttr) {
        if ino == ROOT_INO {
            reply.attr(
                &TTL,
                &FileAttr {
                    ino: 1,
                    size: 0,
                    blocks: 0,
                    atime: UNIX_EPOCH,
                    mtime: UNIX_EPOCH,
                    ctime: UNIX_EPOCH,
                    crtime: UNIX_EPOCH,
                    kind: FileType::Directory,
                    perm: 0o555,
                    nlink: 3,
                    uid: 0,
                    gid: 0,
                    rdev: 0,
                    blksize: 0,
                    padding: 0,
                    flags: 0,
                },
            );
        } else if ino & PROJECTS_INO != 0 {
            self.projects.getattr(ino, reply);
        } else if ino & GROUPS_INO != 0 {
            self.groups.getattr(ino, reply);
        } else {
            reply.error(ENOENT);
        }
    }

    fn lookup(&mut self, _: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let name = name.to_str().unwrap();
        if parent == ROOT_INO {
            match name {
                PROJECTS_STR => self.projects.lookup(parent, name, reply),
                GROUPS_STR => if let Some(reply) = self.groups.lookup(parent, name, reply) {
                    self.projects.lookup(parent, name, reply);
                },
                _ => reply.error(ENOENT),
            }
        } else if parent & PROJECTS_INO != 0 {
            self.projects.lookup(parent, name, reply);
        } else if parent & GROUPS_INO != 0 {
            if let Some(reply) = self.groups.lookup(parent, name, reply) {
                self.projects.lookup(parent, name, reply);
            }
        } else {
            reply.error(ENOENT);
        }
    }

    fn readdir(&mut self, _: &Request, ino: u64, _: u64, offset: i64, mut reply: ReplyDirectory) {
        if ino == ROOT_INO {
            const ENTRIES: [(u64, &str); 2] = [
                (GROUPS_INO, GROUPS_STR),
                (PROJECTS_INO, PROJECTS_STR),
            ];

            for (idx, (ino, name)) in ENTRIES.iter().enumerate().skip(offset as usize) {
                if reply.add(*ino, (idx + 1) as i64, FileType::Symlink, name) {
                    break;
                }
            }

            reply.ok();
        } else if ino & PROJECTS_INO != 0 {
            self.projects.readdir(ino, offset as usize, reply);
        } else if ino & GROUPS_INO != 0 {
            self.groups.readdir(ino, offset as usize, reply);
        } else {
            reply.error(ENOENT);
        }
    }

    fn readlink(&mut self, _: &Request, ino: u64, reply: ReplyData) {
        self.projects.readlink(ino, reply);
    }
}
