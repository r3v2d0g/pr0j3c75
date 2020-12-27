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
use fuser::{FileAttr, FileType, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry};
use libc::ENOENT;
use std::collections::HashMap;
use std::ffi::CString;
use std::time::UNIX_EPOCH;

/* ┌────────────────────────────────────────────────────────────────────────────────────────────┐ *\
 * │                                           const                                            │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

pub const INO: u64 = 1 << 62;
pub const STR: &str = "projects";

/* ┌────────────────────────────────────────────────────────────────────────────────────────────┐ *\
 * │                                          Projects                                          │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

pub struct Projects {
    ino: u64,
    by_name: HashMap<String, u64>,
    by_ino: HashMap<u64, Project>,
}

impl Projects {
/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                       new()                                        │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    pub fn new() -> Self {
        Projects {
            ino: INO,
            by_name: HashMap::new(),
            by_ino: HashMap::new(),
        }
    }

/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                   add_project()                                    │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    pub fn add_project(
        &mut self,
        name: String,
        path: String,
        aliases: impl Iterator<Item = String>,
    ) -> u64 {
        self.ino += 1;

        self.by_name.insert(name.clone(), self.ino).expect_none("project already exists");
        self.by_ino.insert(self.ino, Project {
            path: CString::new(path).unwrap(),
        });

        for alias in aliases {
            self.add_alias(alias, self.ino);
        }

        self.ino
    }

/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                    add_alias()                                     │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    fn add_alias(&mut self, name: String, ino: u64) {
        self.by_name.insert(name, ino).expect_none("project already exists");
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

    pub fn lookup(&self, parent: u64, name: &str, reply: ReplyEntry) {
        let ino = if parent == ROOT_INO {
            INO
        } else if let Some(ino) = self.by_name.get(name) {
            *ino
        } else {
            return reply.error(ENOENT);
        };

        reply.entry(&TTL, &self.attr(ino).unwrap(), 0);
    }

/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                     readdir()                                      │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    pub fn readdir(&self, ino: u64, offset: usize, mut reply: ReplyDirectory) {
        if ino != INO {
            return reply.error(ENOENT);
        }

        for (idx, (name, ino)) in self.by_name.iter().enumerate().skip(offset) {
            if reply.add(*ino, (idx + 1) as i64, FileType::Symlink, name) {
                break;
            }
        }

        reply.ok();
    }

/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                     readlink()                                     │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    pub fn readlink(&self, ino: u64, reply: ReplyData) {
        match self.by_ino.get(&ino) {
            Some(Project { path, .. }) => reply.data(path.as_bytes()),
            None => reply.error(ENOENT),
        }
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
        } else if self.by_ino.contains_key(&ino) {
            Some(FileAttr {
                ino,
                size: 0,
                blocks: 0,
                atime: UNIX_EPOCH,
                mtime: UNIX_EPOCH,
                ctime: UNIX_EPOCH,
                crtime: UNIX_EPOCH,
                kind: FileType::Symlink,
                perm: 0o555,
                nlink: 1,
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
 * │                                          Project                                           │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

#[derive(Debug)]
struct Project {
    path: CString,
}
