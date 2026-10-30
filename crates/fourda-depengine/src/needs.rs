// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! The `needs` round-trip (single-engine design §3.2, C0.4): how a pure rule
//! reads the host's files without reaching a file system.
//!
//! A rule runs against a [`Provided`] store of answers. Whatever it asks for
//! that the store does not hold yet is recorded as a [`Need`] and read as
//! absent for the rest of that run. [`lookup`] then returns either the result
//! ([`Lookup::Done`]: nothing was missing, so the answer is final) or the
//! needs ([`Lookup::Needs`]). The host reads those (a file, a directory
//! listing, cargo's checkout of a crate release, the `package-lock.json`
//! above a project), adds the answers, and calls again. Each call recomputes
//! from scratch, so the final answer is exactly what the rule says with every
//! answer in hand.
//!
//! An answer is `Some(..)` or `None` (the host looked: absent or unreadable).
//! Once a host has run [`MAX_ROUNDS`] rounds it sets [`Provided::exhausted`]
//! and stops answering. A need still unanswered then is never read as clean:
//! the run ends in [`Lookup::Partial`], which names every such need so the
//! host can say NOT CHECKED for what rests on it. The value reads each of
//! them as absent, and each rule says what that costs: the fix path falls
//! back to the semver inference (`Basis::Semver`, "inferred") where a parent's
//! manifest is missing, and names no parent from a requirement range alone
//! where the `package-lock.json` tree went unanswered
//! ([`crate::parent_hint::parent_links_lookup`]).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

/// Rounds of needs a host answers before it sets [`Provided::exhausted`].
/// Three, not the design's two: the fix path's first round can only guess
/// which parents lead to a copy (their ranges); the `package-lock.json` tree
/// answered in that round can name a parent whose manifest is asked in the
/// second, and a pnpm store entry under it is then listed in the second and
/// read in the third. The app never reaches the cap.
pub const MAX_ROUNDS: usize = 3;

/// One thing the engine asks the host for. Paths are the host's own
/// spelling, built with the host's join (the app's is `std::path::Path::join`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Need {
    /// A file's text: an installed parent's `package.json`.
    Manifest { path: String },
    /// The entry names directly in a directory (pnpm's `node_modules/.pnpm`).
    Listing { dir: String },
    /// A crates.io release's own `Cargo.toml`, as cargo extracted it
    /// (`$CARGO_HOME/registry/src/<index>/<name>-<version>`). Where that
    /// checkout lives is the host's to know.
    CrateManifest { name: String, version: String },
    /// The `package-lock.json` that owns the npm tree of the project at
    /// `dir`: the nearest one up the directory ascent the host's tool
    /// detection takes. Absent when pnpm or yarn owns the tree.
    NpmLock { dir: String },
}

/// The host's answers so far. A key present with `None` means "looked, not
/// there"; a key missing means "not asked yet".
#[derive(Debug, Clone, Default)]
pub struct Provided {
    /// [`Need::Manifest`], by path.
    pub manifests: BTreeMap<String, Option<String>>,
    /// [`Need::Listing`], by directory, in the host's listing order.
    pub listings: BTreeMap<String, Option<Vec<String>>>,
    /// [`Need::CrateManifest`], by `name@version`.
    pub crate_manifests: BTreeMap<String, Option<String>>,
    /// [`Need::NpmLock`], by project directory: the parsed lockfile.
    pub npm_locks: BTreeMap<String, Option<Arc<Value>>>,
    /// The host stops answering: every unanswered need reads as absent.
    pub exhausted: bool,
}

impl Provided {
    /// Record the answer to `need` (`None`: the host found nothing).
    pub fn answer(&mut self, need: &Need, found: Answer) {
        match (need, found) {
            (Need::Manifest { path }, Answer::Text(text)) => {
                self.manifests.insert(path.clone(), text);
            }
            (Need::Listing { dir }, Answer::Names(names)) => {
                self.listings.insert(dir.clone(), names);
            }
            (Need::CrateManifest { name, version }, Answer::Text(text)) => {
                self.crate_manifests
                    .insert(crate_manifest_key(name, version), text);
            }
            (Need::NpmLock { dir }, Answer::Tree(tree)) => {
                self.npm_locks.insert(dir.clone(), tree);
            }
            // An answer of the wrong shape is no answer: the need reads as
            // absent, the same as a host that found nothing.
            (Need::Manifest { path }, _) => {
                self.manifests.insert(path.clone(), None);
            }
            (Need::Listing { dir }, _) => {
                self.listings.insert(dir.clone(), None);
            }
            (Need::CrateManifest { name, version }, _) => {
                self.crate_manifests
                    .insert(crate_manifest_key(name, version), None);
            }
            (Need::NpmLock { dir }, _) => {
                self.npm_locks.insert(dir.clone(), None);
            }
        }
    }
}

/// What a host found for one [`Need`].
#[derive(Debug, Clone)]
pub enum Answer {
    /// A file's text ([`Need::Manifest`], [`Need::CrateManifest`]).
    Text(Option<String>),
    /// A directory's entry names ([`Need::Listing`]).
    Names(Option<Vec<String>>),
    /// A parsed `package-lock.json` ([`Need::NpmLock`]).
    Tree(Option<Arc<Value>>),
}

/// The key of a [`Need::CrateManifest`] answer in [`Provided::crate_manifests`].
pub fn crate_manifest_key(name: &str, version: &str) -> String {
    format!("{name}@{version}")
}

/// A rule's answer, or what it still needs from the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup<T> {
    /// Every need the rule had was answered: the answer is final.
    Done(T),
    /// Answer these and call again. Sorted and deduplicated.
    Needs(Vec<Need>),
    /// The store is exhausted and `unanswered` (sorted, deduplicated) never
    /// got an answer. `value` reads each of them as absent, so whatever rests
    /// on one of them was NOT CHECKED; it is never an all-clear.
    Partial { value: T, unanswered: Vec<Need> },
}

/// The host as a rule sees it during one run: the answers held, the host's
/// path join, and the needs recorded so far.
pub struct Host<'a> {
    provided: &'a Provided,
    join: &'a dyn Fn(&str, &str) -> String,
    asked: RefCell<BTreeSet<Need>>,
    /// Needs met once the store is exhausted: never answered, read as absent.
    unanswered: RefCell<BTreeSet<Need>>,
}

impl Host<'_> {
    /// `dir` joined with `name` in the host's spelling.
    pub fn join(&self, dir: &str, name: &str) -> String {
        (self.join)(dir, name)
    }

    fn ask<'h, T>(
        &self,
        held: Option<&'h Option<T>>,
        need: impl FnOnce() -> Need,
    ) -> Option<&'h T> {
        if let Some(answer) = held {
            return answer.as_ref();
        }
        let need = need();
        if self.provided.exhausted {
            self.unanswered.borrow_mut().insert(need);
        } else {
            self.asked.borrow_mut().insert(need);
        }
        None
    }

    /// Whether the host has answered [`Need::NpmLock`] for `dir` (found or
    /// not there). Asks for nothing: a rule uses it to tell "no tree" from "no
    /// answer yet", which must not name what only the tree could confirm.
    pub fn npm_lock_answered(&self, dir: &str) -> bool {
        self.provided.npm_locks.contains_key(dir)
    }

    /// The text of the file at `path`.
    pub fn manifest(&self, path: &str) -> Option<&str> {
        self.ask(self.provided.manifests.get(path), || Need::Manifest {
            path: path.to_string(),
        })
        .map(String::as_str)
    }

    /// The entry names in `dir`, in the host's order.
    pub fn listing(&self, dir: &str) -> Option<&[String]> {
        self.ask(self.provided.listings.get(dir), || Need::Listing {
            dir: dir.to_string(),
        })
        .map(Vec::as_slice)
    }

    /// cargo's extracted `Cargo.toml` of crate `name` at `version`.
    pub fn crate_manifest(&self, name: &str, version: &str) -> Option<&str> {
        self.ask(
            self.provided
                .crate_manifests
                .get(&crate_manifest_key(name, version)),
            || Need::CrateManifest {
                name: name.to_string(),
                version: version.to_string(),
            },
        )
        .map(String::as_str)
    }

    /// The parsed `package-lock.json` owning the npm tree at `dir`.
    pub fn npm_lock(&self, dir: &str) -> Option<&Value> {
        self.ask(self.provided.npm_locks.get(dir), || Need::NpmLock {
            dir: dir.to_string(),
        })
        .map(Arc::as_ref)
    }
}

/// Run `rule` against `provided`: [`Lookup::Done`] when it asked for nothing
/// the store lacks; [`Lookup::Partial`] when the store is exhausted and it
/// met needs nobody answered; else every need it recorded.
pub fn lookup<T>(
    provided: &Provided,
    join: &dyn Fn(&str, &str) -> String,
    rule: impl FnOnce(&Host<'_>) -> T,
) -> Lookup<T> {
    let host = Host {
        provided,
        join,
        asked: RefCell::new(BTreeSet::new()),
        unanswered: RefCell::new(BTreeSet::new()),
    };
    let value = rule(&host);
    let asked = host.asked.into_inner();
    let unanswered = host.unanswered.into_inner();
    if !asked.is_empty() {
        Lookup::Needs(asked.into_iter().collect())
    } else if unanswered.is_empty() {
        Lookup::Done(value)
    } else {
        Lookup::Partial {
            value,
            unanswered: unanswered.into_iter().collect(),
        }
    }
}

/// The host's side of the round-trip: call `rule` until it is done, answering
/// each round's needs with `answer`. After [`MAX_ROUNDS`] rounds the store is
/// exhausted, so the next call ends the loop: at most `MAX_ROUNDS + 1` calls.
/// A need left unanswered there reads as absent, which for the app is what an
/// unreadable file always read as. The app reads the files here; a wasm host
/// runs the same loop in its own language (`api`'s `parent_links`, whose
/// `round` puts the same cap on the engine's side).
pub fn resolve<T>(rule: impl Fn(&Provided) -> Lookup<T>, answer: impl Fn(&Need) -> Answer) -> T {
    let mut provided = Provided::default();
    let mut rounds = 0;
    loop {
        match rule(&provided) {
            Lookup::Done(value) | Lookup::Partial { value, .. } => return value,
            Lookup::Needs(needs) => {
                for need in &needs {
                    provided.answer(need, answer(need));
                }
                rounds += 1;
                if rounds >= MAX_ROUNDS {
                    provided.exhausted = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slash(d: &str, n: &str) -> String {
        format!("{d}/{n}")
    }

    #[test]
    fn a_rule_asks_once_per_missing_answer_and_finishes_when_answered() {
        let rule = |p: &Provided| {
            lookup(p, &slash, |h| {
                let a = h.manifest("/p/a.json").map(str::to_string);
                let b = h.manifest("/p/a.json").map(str::to_string);
                let names = h.listing("/p/.pnpm").map(<[String]>::to_vec);
                (a, b, names)
            })
        };
        let mut provided = Provided::default();
        assert_eq!(
            rule(&provided),
            Lookup::Needs(vec![
                Need::Manifest {
                    path: "/p/a.json".into()
                },
                Need::Listing {
                    dir: "/p/.pnpm".into()
                },
            ])
        );
        provided.answer(
            &Need::Manifest {
                path: "/p/a.json".into(),
            },
            Answer::Text(Some("{}".into())),
        );
        provided.answer(
            &Need::Listing {
                dir: "/p/.pnpm".into(),
            },
            Answer::Names(None),
        );
        assert_eq!(
            rule(&provided),
            Lookup::Done((Some("{}".into()), Some("{}".into()), None))
        );
    }

    #[test]
    fn an_exhausted_store_reads_every_unanswered_need_as_absent_and_names_it() {
        let mut provided = Provided {
            exhausted: true,
            ..Provided::default()
        };
        // Answered "not there" before the cap: absent, and not unanswered.
        provided.manifests.insert("/p/a.json".into(), None);
        let got = lookup(&provided, &slash, |h| {
            (
                h.crate_manifest("mio", "0.8.0").is_none(),
                h.npm_lock("/p").is_none(),
                h.manifest("/p/a.json").is_none(),
                h.npm_lock_answered("/p"),
            )
        });
        assert_eq!(
            got,
            Lookup::Partial {
                value: (true, true, true, false),
                unanswered: vec![
                    Need::CrateManifest {
                        name: "mio".into(),
                        version: "0.8.0".into()
                    },
                    Need::NpmLock { dir: "/p".into() },
                ],
            }
        );
        // Everything asked was answered: an exhausted store is still Done.
        let got = lookup(&provided, &slash, |h| h.manifest("/p/a.json").is_none());
        assert_eq!(got, Lookup::Done(true));
    }

    #[test]
    fn resolve_answers_each_round_and_stops_at_the_cap() {
        // Each round asks for one more file than the last: the cap ends it.
        let rule = |p: &Provided| {
            lookup(p, &slash, |h| {
                (0..10)
                    .take_while(|i| h.manifest(&h.join("/p", &i.to_string())).is_some())
                    .count()
            })
        };
        let read = resolve(rule, |_| Answer::Text(Some(String::new())));
        assert_eq!(read, MAX_ROUNDS);
        // An answer of the wrong shape reads as absent.
        let read = resolve(rule, |_| Answer::Names(Some(Vec::new())));
        assert_eq!(read, 0);
    }
}
