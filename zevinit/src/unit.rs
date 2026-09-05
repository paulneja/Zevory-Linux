// SPDX-License-Identifier: GPL-3.0-or-later

use serde::Deserialize;
use std::fmt;

pub const SUFFIX: &str = ".toml";

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    #[default]
    Simple,
    Oneshot,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Restart {
    #[default]
    Never,
    OnFailure,
    Always,
}

impl Restart {
    pub fn wants_another_try(self, exited_cleanly: bool) -> bool {
        match self {
            Restart::Never => false,
            Restart::OnFailure => !exited_cleanly,
            Restart::Always => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    pub name: String,
    pub description: String,
    pub requires: Vec<String>,
    pub wants: Vec<String>,
    pub after: Vec<String>,
    pub before: Vec<String>,
    pub conflicts: Vec<String>,
    pub kind: Kind,
    pub start: String,
    pub stop: Option<String>,
    pub restart: Restart,
    pub restart_delay: u64,
    pub restart_limit: u32,
    pub start_timeout: u64,
    pub stop_timeout: u64,
    pub directory: Option<String>,
    pub environment: Vec<String>,
}

impl Unit {
    pub fn parse(name: &str, source: &str, text: &str) -> Result<Unit, Problem> {
        let document: Document = toml::from_str(text).map_err(|e| Problem {
            source: source.to_owned(),
            at: e.span().and_then(|s| Spot::of_byte(text, s.start)),
            message: useful_part_of(&e.to_string()),
        })?;

        let unit = Unit {
            name: name.to_owned(),
            description: document.unit.description,
            requires: document.unit.requires,
            wants: document.unit.wants,
            after: document.unit.after,
            before: document.unit.before,
            conflicts: document.unit.conflicts,
            kind: document.service.kind,
            start: document.service.start,
            stop: document.service.stop,
            restart: document.service.restart,
            restart_delay: document.service.restart_delay,
            restart_limit: document.service.restart_limit,
            start_timeout: document.service.start_timeout,
            stop_timeout: document.service.stop_timeout,
            directory: document.service.directory,
            environment: document.service.environment,
        };
        unit.check(source, text)?;
        Ok(unit)
    }

    pub fn name_from_file(file: &str) -> Option<&str> {
        file.strip_suffix(SUFFIX).filter(|stem| !stem.is_empty())
    }

    pub fn needs(&self) -> impl Iterator<Item = &String> {
        self.requires.iter().chain(self.wants.iter())
    }

    fn relations(&self) -> [(&'static str, &Vec<String>); 5] {
        [
            ("requires", &self.requires),
            ("wants", &self.wants),
            ("after", &self.after),
            ("before", &self.before),
            ("conflicts", &self.conflicts),
        ]
    }

    fn check(&self, source: &str, text: &str) -> Result<(), Problem> {
        let complain = |message: String, needle: &str| Problem {
            source: source.to_owned(),
            at: Spot::of_needle(text, needle),
            message,
        };

        if self.start.trim().is_empty() {
            return Err(complain(
                "start is empty, so there is nothing to run".to_owned(),
                "start",
            ));
        }

        if self.description.trim().is_empty() {
            return Err(complain(
                "description is empty, and it is what people see in status".to_owned(),
                "description",
            ));
        }

        for (field, names) in self.relations() {
            for (index, name) in names.iter().enumerate() {
                if name.trim().is_empty() {
                    return Err(complain(format!("{field} has an empty name in it"), field));
                }
                if name == &self.name {
                    return Err(complain(
                        format!("{field} lists {name}, which is this unit itself"),
                        field,
                    ));
                }
                if names[..index].contains(name) {
                    return Err(complain(format!("{field} lists {name} twice"), field));
                }
            }
        }

        for name in &self.conflicts {
            if self.requires.contains(name) || self.wants.contains(name) {
                return Err(complain(
                    format!("{name} is in conflicts and also in requires or wants"),
                    "conflicts",
                ));
            }
        }

        if self.restart != Restart::Never && self.restart_limit == 0 {
            return Err(complain(
                "restart_limit is 0, which asks for restarts and forbids them at once".to_owned(),
                "restart_limit",
            ));
        }

        for (field, seconds) in [
            ("start_timeout", self.start_timeout),
            ("stop_timeout", self.stop_timeout),
        ] {
            if seconds == 0 {
                return Err(complain(
                    format!("{field} is 0, which would give up before the process starts"),
                    field,
                ));
            }
        }

        for entry in &self.environment {
            if !entry.contains('=') {
                return Err(complain(
                    format!("environment entry {entry:?} has no '=', expected NAME=value"),
                    "environment",
                ));
            }
        }

        if self.kind == Kind::Oneshot && self.restart == Restart::Always {
            return Err(complain(
                "a oneshot service with restart = \"always\" would never stop running".to_owned(),
                "restart",
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    unit: Meta,
    service: Service,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Meta {
    description: String,
    #[serde(default)]
    requires: Vec<String>,
    #[serde(default)]
    wants: Vec<String>,
    #[serde(default)]
    after: Vec<String>,
    #[serde(default)]
    before: Vec<String>,
    #[serde(default)]
    conflicts: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Service {
    start: String,
    #[serde(default)]
    stop: Option<String>,
    #[serde(default)]
    kind: Kind,
    #[serde(default)]
    restart: Restart,
    #[serde(default = "one_second")]
    restart_delay: u64,
    #[serde(default = "five_tries")]
    restart_limit: u32,
    #[serde(default = "thirty_seconds")]
    start_timeout: u64,
    #[serde(default = "ten_seconds")]
    stop_timeout: u64,
    #[serde(default)]
    directory: Option<String>,
    #[serde(default)]
    environment: Vec<String>,
}

fn one_second() -> u64 {
    1
}

fn five_tries() -> u32 {
    5
}

fn thirty_seconds() -> u64 {
    30
}

fn ten_seconds() -> u64 {
    10
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    source: String,
    at: Option<Spot>,
    message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Spot {
    line: usize,
    column: usize,
    text: String,
}

impl Spot {
    fn of_byte(text: &str, byte: usize) -> Option<Spot> {
        let upto = text.get(..byte)?;
        let line = upto.matches('\n').count() + 1;
        let column = upto
            .rsplit('\n')
            .next()
            .map_or(1, |t| t.chars().count() + 1);
        let text = text.lines().nth(line - 1)?.to_owned();
        Some(Spot { line, column, text })
    }

    fn of_needle(text: &str, needle: &str) -> Option<Spot> {
        let line = text
            .lines()
            .position(|l| l.trim_start().starts_with(needle))?;
        Some(Spot {
            line: line + 1,
            column: 1,
            text: text.lines().nth(line)?.to_owned(),
        })
    }
}

fn useful_part_of(message: &str) -> String {
    let noise = |line: &&str| {
        let trimmed = line.trim();
        trimmed.is_empty()
            || trimmed.starts_with('|')
            || trimmed.starts_with('^')
            || trimmed.starts_with("TOML parse error")
            || trimmed
                .split_once('|')
                .is_some_and(|(before, _)| before.trim().parse::<u32>().is_ok())
    };
    message
        .lines()
        .rfind(|l| !noise(l))
        .unwrap_or(message)
        .trim()
        .to_owned()
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(spot) = &self.at else {
            return write!(f, "{}: {}", self.source, self.message);
        };
        let gutter = spot.line.to_string();
        let blank = " ".repeat(gutter.len());
        write!(
            f,
            "{}:{}:{}: {}\n{blank} |\n{gutter} | {}\n{blank} | {}^",
            self.source,
            spot.line,
            spot.column,
            self.message,
            spot.text,
            " ".repeat(spot.column.saturating_sub(1))
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Kind, Restart, Unit};

    const SMALLEST: &str = r#"
[unit]
description = "Mount /dev"
[service]
start = "/bin/mount -t devtmpfs devtmpfs /dev"
"#;

    fn parse(text: &str) -> Result<Unit, String> {
        Unit::parse("test", "units/test.toml", text).map_err(|e| e.to_string())
    }

    fn document(unit_extra: &str, service_extra: &str) -> String {
        format!(
            "[unit]\ndescription = \"Mount /dev\"\n{unit_extra}\n\
             [service]\nstart = \"/bin/mount -t devtmpfs devtmpfs /dev\"\n{service_extra}\n"
        )
    }

    fn smallest_with(extra: &str) -> Result<Unit, String> {
        parse(&document("", extra))
    }

    fn related_by(extra: &str) -> Result<Unit, String> {
        parse(&document(extra, ""))
    }

    #[test]
    fn the_shipped_examples_all_parse() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("units");
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).expect("units/ ships with the crate") {
            let path = entry.expect("a readable entry").path();
            let file = path
                .file_name()
                .expect("a name")
                .to_string_lossy()
                .into_owned();
            let name = Unit::name_from_file(&file).expect("examples end in .toml");
            let text = std::fs::read_to_string(&path).expect("a readable unit");
            Unit::parse(name, &file, &text).unwrap_or_else(|e| panic!("{e}"));
            seen += 1;
        }
        assert!(
            seen >= 3,
            "only {seen} example units, the schema needs exercise"
        );
    }

    #[test]
    fn a_minimal_unit_gets_the_documented_defaults() {
        let unit = parse(SMALLEST).expect("the smallest unit is valid");
        assert_eq!(unit.kind, Kind::Simple);
        assert_eq!(unit.restart, Restart::Never);
        assert_eq!(unit.restart_delay, 1);
        assert_eq!(unit.restart_limit, 5);
        assert_eq!(unit.start_timeout, 30);
        assert_eq!(unit.stop_timeout, 10);
        assert!(unit.requires.is_empty());
        assert_eq!(unit.stop, None);
    }

    #[test]
    fn the_name_comes_from_the_file_not_the_contents() {
        assert_eq!(Unit::name_from_file("zevlog.toml"), Some("zevlog"));
        assert_eq!(Unit::name_from_file("zevlog.conf"), None);
        assert_eq!(Unit::name_from_file(".toml"), None);
    }

    #[test]
    fn a_typo_in_a_key_is_refused_and_located() {
        let broken =
            "[unit]\ndescription = \"x\"\nrequiers = []\n[service]\nstart = \"/bin/true\"\n";
        let complaint = parse(broken).expect_err("requiers is not a field");
        assert!(complaint.contains("units/test.toml:3:"), "{complaint}");
        assert!(complaint.contains("unknown field"), "{complaint}");
        assert!(complaint.contains("requiers = []"), "{complaint}");
    }

    #[test]
    fn a_missing_start_is_refused() {
        let complaint = parse("[unit]\ndescription = \"x\"\n[service]\n")
            .expect_err("a service with nothing to run is not a service");
        assert!(complaint.contains("start"), "{complaint}");
    }

    #[test]
    fn an_empty_start_says_so_plainly() {
        let blank = "[unit]\ndescription = \"x\"\n[service]\nstart = \"   \"\n";
        let complaint = parse(blank).expect_err("blank start");
        assert!(complaint.contains("nothing to run"), "{complaint}");
    }

    #[test]
    fn a_unit_cannot_depend_on_itself() {
        let complaint = related_by("after = [\"test\"]").expect_err("self reference");
        assert!(complaint.contains("this unit itself"), "{complaint}");
    }

    #[test]
    fn a_relation_listed_twice_is_refused() {
        let complaint = related_by("wants = [\"a\", \"a\"]").expect_err("duplicate");
        assert!(complaint.contains("twice"), "{complaint}");
    }

    #[test]
    fn requiring_and_conflicting_with_the_same_unit_is_refused() {
        let complaint =
            related_by("requires = [\"a\"]\nconflicts = [\"a\"]").expect_err("contradiction");
        assert!(
            complaint.contains("conflicts and also in requires"),
            "{complaint}"
        );
    }

    #[test]
    fn asking_for_restarts_with_a_limit_of_zero_is_refused() {
        let complaint =
            smallest_with("restart = \"always\"\nrestart_limit = 0").expect_err("contradiction");
        assert!(complaint.contains("forbids them at once"), "{complaint}");
    }

    #[test]
    fn a_zero_timeout_is_refused() {
        let complaint = smallest_with("start_timeout = 0").expect_err("zero timeout");
        assert!(complaint.contains("start_timeout"), "{complaint}");
    }

    #[test]
    fn an_environment_entry_without_an_equals_sign_is_refused() {
        let complaint = smallest_with("environment = [\"PATH\"]").expect_err("no equals");
        assert!(complaint.contains("NAME=value"), "{complaint}");
    }

    #[test]
    fn a_oneshot_that_always_restarts_is_a_contradiction() {
        let complaint = smallest_with("kind = \"oneshot\"\nrestart = \"always\"")
            .expect_err("oneshot cannot run forever");
        assert!(complaint.contains("never stop running"), "{complaint}");
    }

    #[test]
    fn the_restart_policies_mean_what_they_say() {
        assert!(!Restart::Never.wants_another_try(false));
        assert!(!Restart::OnFailure.wants_another_try(true));
        assert!(Restart::OnFailure.wants_another_try(false));
        assert!(Restart::Always.wants_another_try(true));
    }

    #[test]
    fn the_spelling_of_the_enums_is_what_the_files_use() {
        let unit = smallest_with("kind = \"oneshot\"\nrestart = \"on-failure\"")
            .expect("kebab-case is the spelling in the files");
        assert_eq!(unit.kind, Kind::Oneshot);
        assert_eq!(unit.restart, Restart::OnFailure);
    }

    #[test]
    fn a_complaint_points_at_a_line_and_shows_it() {
        let complaint = smallest_with("start_timeout = 0").expect_err("zero timeout");
        assert!(complaint.contains("start_timeout = 0"), "{complaint}");
        assert!(complaint.contains('^'), "{complaint}");
    }
}
