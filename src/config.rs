use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug, Default)]
struct Ignore {
    processes: Box<[String]>,
    users: Box<[String]>,
}

#[derive(Debug, Default)]
struct Rename {
    dictionary: HashMap<String, String>,
}

#[derive(Debug, Default)]
pub struct Config {
    ignore: Ignore,
    rename: Rename,
}

impl Config {
    pub fn is_ignored_user(&self, user: &str) -> bool {
        // TODO: This silly allocation seems so dumb to me. Maybe it gets factored out?
        self.ignore.users.contains(&user.to_string())
    }

    pub fn is_ignored_process(&self, proc: &str) -> bool {
        // TODO: This silly allocation seems so dumb to me. Maybe it gets factored out?
        self.ignore.processes.contains(&proc.to_string())
    }

    pub fn get_canonical_name(&self, proc: &str) -> Option<&String> {
        self.rename.dictionary.get(proc)
    }
}

#[derive(Debug, Clone)]
pub enum ParseConfigError {
    ExpectedColon(usize),
    ExpectedRenameArrow(usize),
    UnknownKeyword(usize, String),
    EmptyRest(usize),
}

impl std::fmt::Display for ParseConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseConfigError::ExpectedColon(ln) => {
                write!(f, "expected colon after keyword on line {ln}")
            }
            ParseConfigError::ExpectedRenameArrow(ln) => {
                write!(f, "expected rename-arrow (->) on line {ln}")
            }
            ParseConfigError::UnknownKeyword(ln, kw) => {
                write!(f, "encountered unknown keyword {kw:?} on line {ln}")
            }
            ParseConfigError::EmptyRest(ln) => {
                write!(f, "expected additional information on line {ln}")
            }
        }
    }
}

impl std::error::Error for ParseConfigError {}

impl FromStr for Config {
    type Err = ParseConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut processes = Vec::new();
        let mut users = Vec::new();
        let mut rename = HashMap::new();

        let lines = s.lines();
        for (ln, line) in lines.enumerate() {
            let ln = ln + 1;
            let line = line.trim_start();

            // Ignore empty lines and comments.
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let Some((keyword, rest)) = line.split_once(':') else {
                return Err(Self::Err::ExpectedColon(ln));
            };
            let rest = rest.trim();
            // Check for some malformed cases.
            match rest {
                "" | "->" => return Err(Self::Err::EmptyRest(ln)),
                l if l.starts_with("->") || l.ends_with("->") => {
                    return Err(Self::Err::EmptyRest(ln));
                }
                _ => {}
            }

            let rest_only_arrow = rest == "->"; // An empty case for renaming.
            let rest_missing_part = line.starts_with("->") || line.ends_with("->"); // Missing a part.
            if rest.is_empty() || rest_only_arrow || rest_missing_part {
                return Err(Self::Err::EmptyRest(ln));
            }

            match keyword {
                "ignore-user" => users.push(rest.to_string()),
                "ignore-proc" => processes.push(rest.to_string()),
                "rename-proc" => {
                    let Some((from, to)) = rest.split_once("->") else {
                        return Err(Self::Err::ExpectedRenameArrow(ln));
                    };
                    rename.insert(from.trim().to_string(), to.trim().to_string());
                }
                unknown => return Err(Self::Err::UnknownKeyword(ln, unknown.to_string())),
            }
        }

        Ok(Self {
            ignore: Ignore {
                processes: processes.into_boxed_slice(),
                users: users.into_boxed_slice(),
            },
            rename: Rename { dictionary: rename },
        })
    }
}
