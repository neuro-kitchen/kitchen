//! Who works in the app: people (a name, and optionally an affiliation and an ORCID iD), the one
//! working now, or a guest. What a person does is recorded under their name (the activity log,
//! an app's own records such as a curation log), so the name may end up in files that are shared:
//! a guest leaves no name (`guest`).
//!
//! No passwords: this names who did what on one computer; servers keep their own pairing. The
//! people are kept in a file the app names ([`load`]), JSON, read and written whole; in a browser,
//! where there is no file, for the page's life.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

/// What a guest is recorded as.
pub const GUEST: &str = "guest";

/// One person.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Person {
    /// As recorded ("Ada Lovelace"); unique among the people.
    pub name: String,
    /// Lab or institution (optional).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub affiliation: String,
    /// ORCID iD, `0000-0000-0000-0000` (optional).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub orcid: String,
}

/// The people file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct Book {
    people: Vec<Person>,
    /// The person working now, by name; `None`: a guest.
    current: Option<String>,
}

#[derive(Default)]
struct State {
    book: Book,
    path: Option<PathBuf>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);

fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(State::default))
}

/// Writes the people file (when there is one); a failure is returned, the change is kept.
fn changed(s: &State) -> Result<(), String> {
    GENERATION.fetch_add(1, Ordering::Relaxed);
    let Some(path) = &s.path else { return Ok(()) };
    write(path, &s.book)
}

#[cfg(not(target_family = "wasm"))]
fn write(path: &Path, book: &Book) -> Result<(), String> {
    let text = serde_json::to_string_pretty(book).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(target_family = "wasm")]
fn write(_: &Path, _: &Book) -> Result<(), String> {
    Ok(())
}

/// Reads the people from `path` (none yet: no one, a guest working) and keeps them there.
/// Returns a notice when the file could not be read (it is then left as it is until a change).
pub fn load(path: impl Into<PathBuf>) -> Option<String> {
    let path = path.into();
    #[cfg(not(target_family = "wasm"))]
    let (book, notice) = match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<Book>(&text) {
            Ok(b) => (b, None),
            Err(e) => (Book::default(), Some(format!("{}: {e}", path.display()))),
        },
        Err(_) => (Book::default(), None),
    };
    #[cfg(target_family = "wasm")]
    let (book, notice) = (Book::default(), None);
    with(|s| {
        s.book = book;
        s.path = Some(path);
    });
    GENERATION.fetch_add(1, Ordering::Relaxed);
    notice
}

/// Bumped on every change (redraw when it moves).
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// Everyone, in the order added.
pub fn people() -> Vec<Person> {
    with(|s| s.book.people.clone())
}

/// The person working now; `None`: a guest.
pub fn current() -> Option<Person> {
    with(|s| s.book.current.as_ref().and_then(|n| s.book.people.iter().find(|p| &p.name == n).cloned()))
}

/// The name things are recorded under now: the person's, or [`GUEST`].
pub fn who() -> String {
    current().map_or_else(|| GUEST.to_string(), |p| p.name)
}

/// `name` works now (`None`: a guest). An unknown name is refused.
pub fn use_person(name: Option<&str>) -> Result<(), String> {
    with(|s| {
        if let Some(n) = name
            && !s.book.people.iter().any(|p| p.name == n)
        {
            return Err(format!("no one named {n:?}"));
        }
        s.book.current = name.map(str::to_string);
        changed(s)
    })
}

/// Checks a person before adding or changing one: a name, not the guest's, an ORCID iD as
/// `0000-0000-0000-000X` when given.
fn check(p: &Person) -> Result<(), String> {
    let name = p.name.trim();
    if name.is_empty() {
        return Err("a person needs a name".into());
    }
    if name.eq_ignore_ascii_case(GUEST) {
        return Err(format!("{GUEST:?} is what no one named is recorded as"));
    }
    if !p.orcid.is_empty() && !orcid_ok(&p.orcid) {
        return Err(format!("{:?} is not an ORCID iD (0000-0000-0000-000X)", p.orcid));
    }
    Ok(())
}

/// `0000-0000-0000-000X`: four groups of four digits, the last character a digit or X, with the
/// ISO 7064 11-2 check digit right.
pub fn orcid_ok(id: &str) -> bool {
    let groups: Vec<&str> = id.split('-').collect();
    if groups.len() != 4 || groups.iter().any(|g| g.len() != 4) {
        return false;
    }
    let chars: Vec<char> = groups.concat().chars().collect();
    let (body, last) = chars.split_at(15);
    if !body.iter().all(char::is_ascii_digit) {
        return false;
    }
    let total = body.iter().fold(0u32, |t, c| (t + c.to_digit(10).unwrap_or(0)) * 2);
    let check = (12 - total % 11) % 11;
    let want = if check == 10 { 'X' } else { char::from_digit(check, 10).unwrap_or('?') };
    last[0].to_ascii_uppercase() == want
}

/// Adds `person` (trimmed), working now when `use_now`. A name already taken is refused.
pub fn add(person: Person, use_now: bool) -> Result<(), String> {
    let person = Person {
        name: person.name.trim().to_string(),
        affiliation: person.affiliation.trim().to_string(),
        orcid: person.orcid.trim().to_uppercase(),
    };
    check(&person)?;
    with(|s| {
        if s.book.people.iter().any(|p| p.name == person.name) {
            return Err(format!("{:?} is there already", person.name));
        }
        if use_now {
            s.book.current = Some(person.name.clone());
        }
        s.book.people.push(person);
        changed(s)
    })
}

/// Forgets `name` (working now: a guest works then). What was recorded under the name stays.
pub fn remove(name: &str) -> Result<(), String> {
    with(|s| {
        s.book.people.retain(|p| p.name != name);
        if s.book.current.as_deref() == Some(name) {
            s.book.current = None;
        }
        changed(s)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// People are added, picked, written and read back; the guest is no one; names are unique and
    /// an ORCID iD is checked.
    #[test]
    fn people_are_kept_and_one_works_at_a_time() {
        let dir = std::env::temp_dir().join(format!("kitchen_people_{}", std::process::id()));
        let path = dir.join("people.json");
        let _ = std::fs::remove_file(&path);
        assert_eq!(load(&path), None);
        assert_eq!((who(), current()), (GUEST.to_string(), None));
        add(
            Person {
                name: " Ada ".into(),
                affiliation: "Analytical Engines".into(),
                orcid: "0000-0002-1825-0097".into(),
            },
            true,
        )
        .unwrap();
        add(Person { name: "Grace".into(), ..Default::default() }, false).unwrap();
        assert_eq!(who(), "Ada");
        assert!(add(Person { name: "Ada".into(), ..Default::default() }, false).is_err(), "names are unique");
        assert!(add(Person { name: "Guest".into(), ..Default::default() }, false).is_err());
        assert!(
            add(Person { name: "Bob".into(), orcid: "0000-0002-1825-0098".into(), ..Default::default() }, false)
                .is_err(),
            "a wrong check digit"
        );
        assert!(orcid_ok("0000-0002-9079-593X"));
        use_person(Some("Grace")).unwrap();
        assert!(use_person(Some("Nobody")).is_err());
        assert_eq!(load(&path), None, "read back");
        assert_eq!((who(), people().len()), ("Grace".to_string(), 2));
        remove("Grace").unwrap();
        assert_eq!(who(), GUEST, "the one removed was working: a guest is");
        use_person(None).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
