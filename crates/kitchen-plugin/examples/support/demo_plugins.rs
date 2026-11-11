//! Small plugin programs (shell scripts) for the gallery and the tests: one that answers, one
//! that speaks a newer protocol, one that fails.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

const ECHO: &str = r#"#!/bin/sh
case "$1" in
  info) echo '{"name":"demo-ml-echo","version":"0.1.0","protocol":1,"licence":"MIT","capabilities":["echo","segment"],"runtime":{"ready":true,"detail":"a shell script"},"models":[{"id":"echo-tiny"}]}' ;;
  serve)
    while IFS= read -r line; do
      case "$line" in
        *'"quit"'*) exit 0 ;;
        *'"fail"'*) echo '{"event":"error","message":"asked to fail"}' ;;
        *) echo 'a line that is not an event'; echo '{"event":"progress","stage":"echoing","fraction":0.5}'; printf '{"event":"done","result":%s}\n' "$line" ;;
      esac
    done ;;
  *) echo "usage: info | serve" >&2; exit 2 ;;
esac
"#;

const NEWER: &str = r#"#!/bin/sh
echo '{"name":"demo-ml-future","version":"9.0.0","protocol":2,"licence":"MIT"}'
"#;

const BROKEN: &str = r#"#!/bin/sh
echo "cannot find its runtime" >&2
exit 1
"#;

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    p
}

/// Writes the three plugins into `dir`; returns the answering one.
pub fn write_all(dir: &Path) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    write(dir, "demo-ml-future", NEWER);
    write(dir, "demo-ml-broken", BROKEN);
    write(dir, "demo-ml-echo", ECHO)
}
