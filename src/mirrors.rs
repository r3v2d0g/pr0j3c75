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
use regex::Regex;
use std::collections::HashMap;
use std::ffi::{CString, OsString};
use std::fs;
use std::path::PathBuf;
use std::time::{UNIX_EPOCH, SystemTime};

/* ┌────────────────────────────────────────────────────────────────────────────────────────────┐ *\
 * │                                           const                                            │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

pub const INO: u64 = 1 << 63;
pub const STR: &str = "mirrors";

/* ┌────────────────────────────────────────────────────────────────────────────────────────────┐ *\
 * │                                          Mirrors                                           │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

pub struct Mirrors {
    ino: u64,
    by_name: HashMap<String, u64>,
    by_ino: HashMap<u64, Mirror>,
    rec_by_ino: HashMap<u64, u64>,
}

impl Mirrors {
/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                       new()                                        │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    pub fn new() -> Self {
        Mirrors {
            ino: INO,
            by_name: HashMap::new(),
            by_ino: HashMap::new(),
            rec_by_ino: HashMap::new(),
        }
    }

/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                    add_mirror()                                    │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    pub fn add_mirror(
        &mut self,
        name: String,
        base: String,
        renaming: Regex,
        aliases: impl Iterator<Item = String>,
    ) -> u64 {
        self.ino += 1;

        self.by_name.insert(name.clone(), self.ino).expect_none("mirror already exists");

        self.by_ino.insert(self.ino, Mirror {
            ino: self.ino,
            base: PathBuf::from(base),
            renaming,
            by_name: HashMap::new(),
            by_ino: HashMap::new(),
            modified: UNIX_EPOCH,
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
        self.by_name.insert(name, ino).expect_none("mirror already exists");
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
 *     │                                       lookup                                       │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    pub fn lookup(&self, parent: u64, name: &str, reply: ReplyEntry) {
        if parent == ROOT_INO {
            return reply.entry(&TTL, &self.attr(INO).unwrap(), 0);
        } else if parent == INO {
            if let Some(ino) = self.by_name.get(name).copied() {
                return reply.entry(&TTL, &self.attr(ino).unwrap(), 0);
            }
        } else if let Some(mirror) = self.by_ino.get(&parent) {
            if let Some(attr) = mirror.attr(name) {
                return reply.entry(&TTL, &attr, 0);
            }
        }

        reply.error(ENOENT);
    }

/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                     readdir()                                      │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    pub fn readdir(&mut self, ino: u64, offset: usize, mut reply: ReplyDirectory) {
        if ino == INO {
            for (idx, (name, ino)) in self.by_name.iter().enumerate().skip(offset) {
                if reply.add(*ino, (idx + 1) as i64, FileType::Directory, name) {
                    break;
                }
            }
        } else if let Some(mirror) = self.by_ino.get_mut(&ino) {
            if offset == 0 && mirror.modified.elapsed().unwrap() > TTL {
                mirror.update(&mut self.ino, &mut self.rec_by_ino);
            }

            for (idx, (name, ino)) in mirror.by_name.iter().enumerate().skip(offset) {
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
 *     │                                     readlink()                                     │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    pub fn readlink(&self, ino: u64, reply: ReplyData) {
        if let Some(mino) = self.rec_by_ino.get(&ino) {
            if let Some(Mirror { by_ino, .. }) = self.by_ino.get(mino) {
                if let Some(path) = by_ino.get(&ino) {
                    return reply.data(path.as_bytes());
                }
            }
        }

        reply.error(ENOENT);
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
        } else if let Some(mirror) = self.by_ino.get(&ino) {
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
                nlink: 1 + mirror.by_name.len() as u32,
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
 * │                                           Mirror                                           │ *
\* └────────────────────────────────────────────────────────────────────────────────────────────┘ */

struct Mirror {
    ino: u64,
    base: PathBuf,
    renaming: Regex,
    by_name: HashMap<String, u64>,
    by_ino: HashMap<u64, CString>,
    modified: SystemTime,
}

impl Mirror {
/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                      update()                                      │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    fn update(&mut self, ino: &mut u64, rec_by_ino: &mut HashMap<u64, u64>) {
        let modified = fs::metadata(&self.base).unwrap().modified().unwrap();
        if modified == self.modified {
            return;
        }

        self.modified = modified;

        let mut paths = fs::read_dir(&self.base)
            .unwrap()
            .map(Result::unwrap)
            .filter(|entry| entry.file_type().unwrap().is_dir())
            .map(|entry| entry.file_name())
            .map(OsString::into_string)
            .map(Result::unwrap)
            .filter_map(|name| {
                let renamed = self.renaming.captures(&name)?;
                let renamed = renamed.get(renamed.len() - 1).unwrap();
                let path = self.base.join(&name);
                let path = CString::new(path.to_str().unwrap()).unwrap();
                
                Some((renamed.as_str().to_string(), path))
            })
            .collect::<HashMap<_, _>>();

        let Mirror { ref mut by_name, ref mut by_ino, .. } = self;
        by_name.retain(|name, ino| {
            if let Some(path) = paths.remove(name) {
                by_ino.insert(*ino, path);
                true
            } else {
                by_ino.remove(ino);
                rec_by_ino.remove(ino);

                false
            }
        });

        for (name, path) in paths {
            *ino += 1;

            self.by_name.insert(name, *ino).expect_none("mirror already contains path");
            self.by_ino.insert(*ino, path);
            rec_by_ino.insert(*ino, self.ino);
        }
    }

/*     ┌────────────────────────────────────────────────────────────────────────────────────┐     *\
 *     │                                       attr()                                       │     *
\*     └────────────────────────────────────────────────────────────────────────────────────┘     */

    fn attr(&self, name: &str) -> Option<FileAttr> {
        if let Some(ino) = self.by_name.get(name).copied() {
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
