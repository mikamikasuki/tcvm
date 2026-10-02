//! `os.date` and `os.time(table)`. Expected strings come from `lua` 5.5.1
//! running the same chunk; everything checked is independent of the local
//! timezone (UTC formats, noon times, `isdst` left out).

use tcvm::env::Value;
use tcvm::{Executor, LoadError, Lua, RuntimeError};

fn run(src: &str) -> Result<String, String> {
    let mut lua = Lua::new();
    lua.load_all();
    let ex = lua
        .try_enter(|ctx| -> Result<_, LoadError> {
            let chunk = ctx.load(src, Some("=c"))?;
            Ok(ctx.stash(Executor::start(ctx, chunk, ())))
        })
        .expect("load");
    let as_string = |v: Value<'_>| {
        let s = v.get_string().expect("string value");
        String::from_utf8_lossy(s.as_bytes()).into_owned()
    };
    match lua.finish(&ex) {
        Ok(()) => Ok(lua.enter(|ctx| {
            let v = ctx.fetch(&ex).take_result::<Value>(ctx).expect("result");
            as_string(v)
        })),
        Err(RuntimeError::Lua(e)) => Err(lua.enter(|ctx| as_string(ctx.fetch(&e).value()))),
        Err(e) => panic!("unexpected failure for {src:?}: {e:?}"),
    }
}

fn ok(src: &str) -> String {
    run(src).unwrap_or_else(|e| panic!("{src:?} failed: {e}"))
}

fn err(src: &str) -> String {
    run(src).expect_err(src)
}

#[test]
fn date_formats() {
    assert_eq!(
        ok(r#"return os.date("!%c|%F %T|%a %b %j %p|%Ey %OH %%", 951782400)"#),
        "Tue Feb 29 00:00:00 2000|2000-02-29 00:00:00|Tue Feb 060 AM|00 00 %"
    );
    // Literal bytes, NULs included, pass through; a number is a format too.
    assert_eq!(ok(r#"return os.date("!x\0%Y", 0)"#), "x\x001970");
    assert_eq!(ok("return os.date(2026, 0)"), "2026");
}

#[test]
fn date_table() {
    assert_eq!(
        ok(r#"local t = os.date("!*t", 951782400)
              return string.format("%d-%d-%d %d:%d:%d yday=%d wday=%d isdst=%s",
                  t.year, t.month, t.day, t.hour, t.min, t.sec, t.yday, t.wday,
                  tostring(t.isdst))"#),
        "2000-2-29 0:0:0 yday=60 wday=3 isdst=false"
    );
}

#[test]
fn time_normalizes_table() {
    assert_eq!(
        ok(r#"local t = {year = 2026, month = 13, day = 40}
              local r = os.time(t)
              return string.format("%d-%d-%d %d:%d:%d yday=%d wday=%d %s",
                  t.year, t.month, t.day, t.hour, t.min, t.sec, t.yday, t.wday,
                  tostring(r == os.time(os.date("*t", r))))"#),
        "2027-2-9 12:0:0 yday=40 wday=3 true"
    );
}

#[test]
fn time_goes_through_metamethods() {
    assert_eq!(
        ok(r#"local log = {}
              local t = setmetatable({}, {
                  __index = function(_, k)
                      log[#log + 1] = k
                      return ({year = 2024, month = 2, day = 30})[k]
                  end,
                  __newindex = function(t, k, v)
                      log[#log + 1] = k .. "=" .. tostring(v)
                      rawset(t, k, v)
                  end,
              })
              os.time(t)
              for i = #log, 1, -1 do
                  if log[i]:find("^isdst=") then table.remove(log, i) end
              end
              return table.concat(log, " ")"#),
        "year month day hour min sec isdst \
         year=2024 month=3 day=1 hour=12 min=0 sec=0 yday=61 wday=6"
    );
}

#[test]
fn errors() {
    assert_eq!(
        err(r#"return os.date("%Ez abc")"#),
        "c:1: bad argument #1 to 'date' (invalid conversion specifier '%Ez abc')"
    );
    assert_eq!(
        err(r#"return os.date("%Y", 1.5)"#),
        "c:1: bad argument #2 to 'date' (number has no integer representation)"
    );
    assert_eq!(
        err(r#"return os.date("%Y", 2^60)"#),
        "c:1: date result cannot be represented in this installation"
    );
    assert_eq!(
        err("return os.time({year = 2000, day = 1})"),
        "c:1: field 'month' missing in date table"
    );
    assert_eq!(
        err("return os.time({year = 2000, month = 1.5, day = 1})"),
        "c:1: field 'month' is not an integer"
    );
    assert_eq!(
        err("return os.time({year = 2^40, month = 1, day = 1})"),
        "c:1: field 'year' is out-of-bound"
    );
    assert_eq!(
        err("return os.time(5)"),
        "c:1: bad argument #1 to 'time' (table expected, got number)"
    );
}
