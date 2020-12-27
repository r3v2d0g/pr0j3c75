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

use super::{TTL, ROOT_INO};
use fuser::{FileAttr, FileType, ReplyAttr, ReplyDirectory, ReplyEntry};
use libc::ENOENT;
use std::collections::{BTreeMap, HashMap};
use std::time::UNIX_EPOCH;

/* ┌────────────────────────────────────────────────────────────────────────────────────────────┐ *\
 * │                                           const                                            │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

pub const INO: u64 = 1 << 62;
pub const STR: &str = "by-group";

/* ┌────────────────────────────────────────────────────────────────────────────────────────────┐ *\
 * │                                           Groups                                           │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

pub struct Groups {
    ino: u64,
    by_name: HashMap<String, u64>,
    by_ino: BTreeMap<u64, Group>,
}

impl Groups {
/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                       new()                                        │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    pub fn new() -> Self {
        Groups {
            ino: INO,
            by_name: HashMap::new(),
            by_ino: BTreeMap::new(),
        }
    }

/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                    add_group()                                     │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    pub fn add_group(&mut self, name: String, projects: HashMap<String, u64>) -> u64 {
        self.ino += 1;
        self.by_name.insert(name.clone(), self.ino);
        self.by_ino.insert(self.ino, Group {
            name,
            by_ino: projects.iter().map(|(name, ino)| (*ino, name.clone())).collect(),
            by_name: projects,
        }).expect_none("group already exists");

        self.ino
    }

/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                    get_group()                                     │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    pub fn get_group(&mut self, ino: u64) -> Option<&mut Group> {
        self.by_ino.get_mut(&ino)
    }

/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                     getattr()                                      │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    pub fn getattr(&self, ino: u64, reply: ReplyAttr) {
        if let Some(attr) = self.attr(ino) {
            reply.attr(&TTL, &attr);
        } else {
            reply.error(ENOENT);
        }
    }

/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                      lookup()                                      │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    pub fn lookup(&self, parent: u64, name: &str, reply: ReplyEntry) -> Option<ReplyEntry> {
        if parent == ROOT_INO {
            reply.entry(&TTL, &self.attr(INO).unwrap(), 0);
        } else if let Some(ino) = self.by_name.get(name) {
            reply.entry(&TTL, &self.attr(*ino).unwrap(), 0);
        } else if self.by_ino.get(&parent).is_some() {
            return Some(reply);
        } else {
            reply.error(ENOENT);
        }

        None
    }

/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                     readdir()                                      │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    pub fn readdir(&self, ino: u64, offset: usize, mut reply: ReplyDirectory) {
        if ino == INO {
            for (idx, (name, ino)) in self.by_name.iter().enumerate().skip(offset) {
                if reply.add(*ino, (idx + 1) as i64, FileType::Directory, name) {
                    break;
                }
            }
        } else if let Some(group) = self.by_ino.get(&ino) {
            for (idx, (name, ino)) in group.by_name.iter().enumerate().skip(offset) {
                if reply.add(*ino, (idx + 1) as i64, FileType::Symlink, name) {
                    break;
                }
            }
        } else {
            return reply.error(ENOENT);
        }

        reply.ok();
    }

/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                       attr()                                       │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    fn attr(&self, ino: u64) -> Option<FileAttr> {
        if ino == INO {
            Some(FileAttr {
                ino,
                size: 0,
                blocks: 0,
                atime: UNIX_EPOCH,
                mtime: UNIX_EPOCH,
                ctime: UNIX_EPOCH,
                crtime: UNIX_EPOCH,
                kind: FileType::Directory,
                perm: 0o555,
                nlink: 1 + self.by_name.len() as u32,
                uid: 0,
                gid: 0,
                rdev: 0,
                blksize: 0,
                padding: 0,
                flags: 0,
            })
        } else if let Some(group) = self.by_ino.get(&ino) {
            Some(FileAttr {
                ino,
                size: 0,
                blocks: 0,
                atime: UNIX_EPOCH,
                mtime: UNIX_EPOCH,
                ctime: UNIX_EPOCH,
                crtime: UNIX_EPOCH,
                kind: FileType::Directory,
                perm: 0o555,
                nlink: 1 + group.by_name.len() as u32,
                uid: 0,
                gid: 0,
                rdev: 0,
                blksize: 0,
                padding: 0,
                flags: 0,
            })
        } else {
            None
        }
    }
}

/* ┌────────────────────────────────────────────────────────────────────────────────────────────┐ *\
 * │                                           Group                                            │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

#[derive(Debug)]
pub struct Group {
    name: String,
    by_name: HashMap<String, u64>,
    by_ino: BTreeMap<u64, String>,
}

impl Group {
/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                   add_project()                                    │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    pub fn add_project(&mut self, name: String, ino: u64) {
        self.by_name.insert(name.clone(), ino);
        self.by_ino.insert(ino, name);
    }

/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                    add_alias()                                     │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    pub fn add_alias(&mut self, name: String, ino: u64) {
        self.by_name.insert(name.clone(), ino);
    }
}
