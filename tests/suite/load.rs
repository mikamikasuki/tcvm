//! `load`. Expected strings come from `lua` 5.5.1 running the same chunk,
//! except where tcvm diverges on purpose: syntax errors are rendered ariadne
//! reports, and only UTF-8 text chunks load.

use tcvm::env::Value;
use tcvm::{Executor, LoadError, Lua, RuntimeError};

/// Kept on the chunk's first line so error positions are unaffected.
const PRELUDE: &str = "local function cat(...) local t = table.pack(...) \
    for i = 1, t.n do t[i] = tostring(t[i]) end return table.concat(t, ' ') end ";

fn run(src: &str) -> String {
    let mut lua = Lua::new();
    lua.load_all();
    let src = format!("{PRELUDE}{src}");
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
fn string_chunk() {
    assert_eq!(
        run(
            r#"local f = load("local a, b = ... return x, a + b, _ENV ~= nil", "=env", "t", {x = "env"})
               return cat(f(1, 2))"#
        ),
        "env 3 true"
    );
    // An explicit nil `env` still replaces `_ENV`.
    assert_eq!(
        run(r#"return cat(load("return _ENV", "=e", "t", nil)())"#),
        "nil"
    );
    assert_eq!(run(r#"return cat(load("return x")())"#), "nil");
}

#[test]
fn reader_function() {
    // Pieces are concatenated (numbers converted) up to the empty string.
    assert_eq!(
        run(r#"local parts, i = {"return ", 4, "2", "", "error()"}, 0
               return cat(load(function() i = i + 1 return parts[i] end)())"#),
        "42"
    );
    assert_eq!(
        run("return cat(load(function() return true end))"),
        "nil c:1: reader function must return a string"
    );
    assert_eq!(
        run(r#"return cat(load(function() error("boom", 0) end))"#),
        "nil boom"
    );
}

#[test]
fn modes() {
    assert_eq!(
        run(r#"return cat(load("return 1", "=m", "b"))"#),
        "nil attempt to load a text chunk (mode is 'b')"
    );
    assert_eq!(
        run(r#"return cat(load("\27Lua", "=m", "t"))"#),
        "nil attempt to load a binary chunk (mode is 't')"
    );
    assert_eq!(
        run(r#"return cat(load("\27Lua", "=m"))"#),
        "nil binary chunks are not supported"
    );
    assert_eq!(
        run(r#"return cat(pcall(load, "return 1", nil, "tB"))"#),
        "false bad argument #3 to 'load' (invalid mode)"
    );
}

#[test]
fn bad_arguments() {
    assert_eq!(
        run("return cat(pcall(load, {}))"),
        "false bad argument #1 to 'load' (function expected, got table)"
    );
    assert_eq!(
        run(r#"return cat(pcall(load, "x", {}))"#),
        "false bad argument #2 to 'load' (string expected, got table)"
    );
}

#[test]
fn errors() {
    assert_eq!(
        run(r#"return cat(load("local x <const> = 1\nx = 2"))"#),
        "nil [string \"local x <const> = 1...\"]:2: attempt to assign to const variable 'x'"
    );
    assert_eq!(
        run(r#"return cat(load("x = = 1"))"#),
        "nil Error: expected a statement\n   \
         ╭─[ [string \"x = = 1\"]:1:5 ]\n   \
         │\n \
         1 │ x = = 1\n   \
         │     ┬  \n   \
         │     ╰── expected a statement but got \"=\"\n\
         ───╯"
    );
    assert_eq!(
        run(r#"return cat(load("return '\255'"))"#),
        "nil chunk is not valid UTF-8"
    );
}

#[test]
fn host_compile_error_names_the_chunk() {
    let mut lua = Lua::new();
    let msg = lua.enter(
        |ctx| match ctx.load("local x <const> = 1\nx = 2", Some("=c")) {
            Ok(_) => panic!("const assignment compiled"),
            Err(e) => e.to_string(),
        },
    );
    assert_eq!(msg, "c:2: attempt to assign to const variable 'x'");
}
