use log::{kv, Level, Record};
use std::{
    fmt,
    io::{self, Write},
};

/// Writes fields into a single logfmt record.
pub struct Logfmt<'a> {
    output: &'a mut dyn Write,
}

impl Logfmt<'_> {
    /// Appends a quoted field. Control characters and quotes are escaped.
    pub fn field(&mut self, key: &str, value: impl fmt::Display) -> io::Result<()> {
        if valid_key(key) {
            write!(self.output, " {key}={}", Quoted(value))?;
        }
        Ok(())
    }

    /// Appends an unquoted unsigned numeric field for log collectors.
    pub fn number(&mut self, key: &str, value: u64) -> io::Result<()> {
        if valid_key(key) {
            write!(self.output, " {key}={value}")?;
        }
        Ok(())
    }
}

pub(super) fn level_name(level: Level) -> &'static str {
    match level {
        Level::Error => "error",
        Level::Warn => "warn",
        Level::Info => "info",
        Level::Debug => "debug",
        Level::Trace => "trace",
    }
}

pub(super) fn write_record(
    output: &mut dyn Write,
    timestamp: impl fmt::Display,
    level: impl fmt::Display,
    record: &Record<'_>,
    context: impl FnOnce(&mut Logfmt<'_>) -> io::Result<()>,
) -> io::Result<()> {
    write!(
        output,
        "ts={} level={level} target={}",
        Quoted(timestamp),
        Quoted(record.target())
    )?;
    let mut fields = Logfmt { output };
    context(&mut fields)?;
    let mut visitor = Fields {
        fields: &mut fields,
        error: None,
    };
    let result = record.key_values().visit(&mut visitor);
    if let Some(error) = visitor.error {
        return Err(error);
    }
    if let Err(error) = result {
        return Err(io::Error::other(error.to_string()));
    }
    writeln!(output, " msg={}", Quoted(record.args()))
}

struct Fields<'a, 'b> {
    fields: &'a mut Logfmt<'b>,
    error: Option<io::Error>,
}

impl<'kvs> kv::VisitSource<'kvs> for Fields<'_, '_> {
    fn visit_pair(&mut self, key: kv::Key<'kvs>, value: kv::Value<'kvs>) -> Result<(), kv::Error> {
        let key = key.as_str();
        if valid_key(key) {
            value.visit(Field { key, fields: self })?;
        }
        Ok(())
    }
}

struct Field<'a, 'b, 'c> {
    key: &'a str,
    fields: &'a mut Fields<'b, 'c>,
}

impl Field<'_, '_, '_> {
    fn scalar(&mut self, value: impl fmt::Display) -> Result<(), kv::Error> {
        let result = write!(self.fields.fields.output, " {}={value}", self.key);
        self.finish(result)
    }

    fn finish(&mut self, result: io::Result<()>) -> Result<(), kv::Error> {
        result.map_err(|error| {
            self.fields.error = Some(error);
            kv::Error::msg("writing logfmt field failed")
        })
    }
}

impl kv::VisitValue<'_> for Field<'_, '_, '_> {
    fn visit_any(&mut self, value: kv::Value<'_>) -> Result<(), kv::Error> {
        let result = self.fields.fields.field(self.key, value);
        self.finish(result)
    }

    fn visit_null(&mut self) -> Result<(), kv::Error> {
        Ok(())
    }

    fn visit_i64(&mut self, value: i64) -> Result<(), kv::Error> {
        self.scalar(value)
    }
    fn visit_u64(&mut self, value: u64) -> Result<(), kv::Error> {
        self.scalar(value)
    }
    fn visit_i128(&mut self, value: i128) -> Result<(), kv::Error> {
        self.scalar(value)
    }
    fn visit_u128(&mut self, value: u128) -> Result<(), kv::Error> {
        self.scalar(value)
    }
    fn visit_f64(&mut self, value: f64) -> Result<(), kv::Error> {
        self.scalar(value)
    }
    fn visit_bool(&mut self, value: bool) -> Result<(), kv::Error> {
        self.scalar(value)
    }
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && !matches!(key, "ts" | "level" | "target" | "msg")
        && key.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
}

struct Quoted<T>(T);

impl<T: fmt::Display> fmt::Display for Quoted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write as _;
        f.write_char('"')?;
        write!(Escaped(f), "{}", self.0)?;
        f.write_char('"')
    }
}

struct Escaped<'a, 'b>(&'a mut fmt::Formatter<'b>);

impl fmt::Write for Escaped<'_, '_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut plain = 0;
        for (index, ch) in s.char_indices() {
            if !matches!(ch, '"' | '\\') && !ch.is_control() {
                continue;
            }
            self.0.write_str(&s[plain..index])?;
            match ch {
                '"' => self.0.write_str("\\\"")?,
                '\\' => self.0.write_str("\\\\")?,
                '\n' => self.0.write_str("\\n")?,
                '\r' => self.0.write_str("\\r")?,
                '\t' => self.0.write_str("\\t")?,
                _ => write!(self.0, "\\u{:04x}", ch as u32)?,
            }
            plain = index + ch.len_utf8();
        }
        self.0.write_str(&s[plain..])
    }
}
