//! `loadfile` and `dofile`. Expected strings come from `lua` 5.5.1 loading
//! the same files.

use std::path::{Path, PathBuf};

use tcvm::env::Value;
use tcvm::{Executor, LoadError, Lua, RuntimeError};

/// Kept on the chunk's first line so error positions are unaffected.
const PRELUDE: &str = "local function cat(...) local t = table.pack(...) \
    for i = 1, t.n do t[i] = tostring(t[i]) end return table.concat(t, ' ') end ";

/// A fresh directory holding `files`, as `(name, contents)`.
fn dir_with(test: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("loadfile")
        .join(test);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, contents) in files {
        std::fs::write(dir.join(name), contents).unwrap();
    }
    dir
}

/// Run `src` with `dir` (ending in `/`) bound to the local `dir`.
fn run(dir: &Path, src: &str) -> String {
    let mut lua = Lua::new();
    lua.load_all();
    let src = format!(
        "{PRELUDE}local dir = {:?} {src}",
        format!("{}/", dir.display())
    );
    let ex = lua
        .try_enter(|ctx| -> Result<_, LoadError> {
            let chunk = ctx.load(&src, Some("=c"))?;
            Ok(ctx.stash(Executor::start(ctx, chunk, ())))
        })
        .expect("load");
    let as_string = |v: Value<'_>| {
        let s = v.get_string().expect("string value");
        String::from_utf8_lossy(s.as_bytes()).into_owned()
    };
    match lua.finish(&ex) {
        Ok(()) => lua.enter(|ctx| {
            let v = ctx.fetch(&ex).take_result::<Value>(ctx).expect("result");
            as_string(v)
        }),
        Err(RuntimeError::Lua(e)) => {
            let msg = lua.enter(|ctx| as_string(ctx.fetch(&e).value()));
            panic!("{src:?} failed: {msg}")
        }
        Err(e) => panic!("unexpected failure for {src:?}: {e:?}"),
    }
}

#[test]
fn skips_bom_and_comment_line() {
    let dir = dir_with(
        "skips",
        &[
            (
                "shebang.lua",
                b"#!/usr/bin/env lua\nlocal a = ...\nreturn a, error(\"e\")\n",
            ),
            ("bom.lua", b"\xEF\xBB\xBFreturn 'bom'\n"),
            ("only_comment.lua", b"# nothing else"),
        ],
    );
    // The skipped line still counts, so the error is on line 3.
    assert_eq!(
        run(
            &dir,
            r#"local ok, e = pcall(loadfile(dir .. "shebang.lua"), 7)
               return cat(ok, e:match(":(%d+): e$"))"#
        ),
        "false 3"
    );
    assert_eq!(
        run(&dir, r#"return cat(loadfile(dir .. "bom.lua")())"#),
        "bom"
    );
    assert_eq!(
        run(&dir, r#"return cat(loadfile(dir .. "only_comment.lua")())"#),
        ""
    );
}

#[test]
fn mode_and_env() {
    let dir = dir_with(
        "mode_env",
        &[("env.lua", b"return x"), ("bin.lua", b"#c\n\x1bLua")],
    );
    assert_eq!(
        run(
            &dir,
            r#"return cat(loadfile(dir .. "env.lua", "t", {x = "E"})())"#
        ),
        "E"
    );
    assert_eq!(
        run(&dir, r#"return cat(loadfile(dir .. "env.lua", "b"))"#),
        "nil attempt to load a text chunk (mode is 'b')"
    );
    assert_eq!(
        run(&dir, r#"return cat(loadfile(dir .. "bin.lua", "t"))"#),
        "nil attempt to load a binary chunk (mode is 't')"
    );
    assert_eq!(
        run(
            &dir,
            r#"return cat(pcall(loadfile, dir .. "env.lua", "B"))"#
        ),
        "false bad argument #2 to 'loadfile' (invalid mode)"
    );
}

#[test]
fn file_errors() {
    let dir = dir_with("file_errors", &[]);
    let missing = format!("{}/missing.lua", dir.display());
    assert_eq!(
        run(&dir, r#"return cat(loadfile(dir .. "missing.lua"))"#),
        format!("nil cannot open {missing}: No such file or directory")
    );
    // A directory opens but can't be read; Lua reports no `strerror` then.
    assert_eq!(
        run(&dir, "return cat(loadfile(dir))"),
        format!("nil cannot read {}/", dir.display())
    );
    assert_eq!(
        run(&dir, "return cat(pcall(loadfile, {}))"),
        "false bad argument #1 to 'loadfile' (string expected, got table)"
    );
}

#[test]
fn dofile_runs_and_raises() {
    let dir = dir_with(
        "dofile",
        &[
            ("multi.lua", b"return 1, 2, ..."),
            ("err.lua", b"error('in file', 0)"),
        ],
    );
    assert_eq!(
        run(&dir, r#"return cat(dofile(dir .. "multi.lua"))"#),
        "1 2"
    );
    assert_eq!(
        run(&dir, r#"return cat(pcall(dofile, dir .. "err.lua"))"#),
        "false in file"
    );
    let missing = format!("{}/missing.lua", dir.display());
    assert_eq!(
        run(&dir, r#"return cat(pcall(dofile, dir .. "missing.lua"))"#),
        format!("false cannot open {missing}: No such file or directory")
    );
}
