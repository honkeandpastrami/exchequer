//! The wording the decision model is shown: Jinja templates in `resources/`,
//! built into the binary and rendered with minijinja. [`crate::context`]
//! computes what they're filled with; a change to either is a new version
//! of the questions ([`crate::QUESTIONS`]).

use std::sync::LazyLock;

use minijinja::value::Serde;
use minijinja::{Environment, UndefinedBehavior};
use serde_json::Value as Json;

macro_rules! templates {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_str!(concat!("../resources/", $name)))),*]
    };
}

const TEMPLATES: &[(&str, &str)] = templates!(
    "labels.j2",
    "macros.j2",
    "questions/counterparty.j2",
    "questions/category.j2",
    "questions/purpose.j2",
    "questions/account.j2",
    "options/counterparty.j2",
    "options/category.j2",
    "options/purpose.j2",
    "options/account.j2",
    "state/recurs.j2",
);

// The templates are part of the source: one that doesn't parse or render
// is a bug, caught by the tests.
static ENV: LazyLock<Environment<'static>> = LazyLock::new(|| {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::SemiStrict);
    for (name, source) in TEMPLATES {
        env.add_template(name, source)
            .unwrap_or_else(|e| panic!("{name}: {e:#}"));
    }
    env
});

/// `name` filled with `ctx`, on one line: templates are laid out to be read,
/// so their runs of whitespace become one space.
pub fn render(name: &str, ctx: &Json) -> String {
    let s = ENV
        .get_template(name)
        .and_then(|t| t.render(Serde(ctx)))
        .unwrap_or_else(|e| panic!("{name}: {e:#}"));
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The names of the options that stand for "none" or for no answer, and
/// what each means (`labels.j2`).
pub struct Labels {
    pub unknown_counterparty: String,
    pub unknown_counterparty_means: String,
    pub no_category: String,
    pub your_account: String,
    pub owner: String,
    pub none_of_these: String,
    pub no_counterparty_listed: String,
    pub no_account_listed: String,
    pub cannot_tell: String,
    pub cannot_tell_means: String,
}

pub static LABELS: LazyLock<Labels> = LazyLock::new(|| {
    let t = ENV
        .get_template("labels.j2")
        .and_then(|t| t.render_captured(()))
        .unwrap_or_else(|e| panic!("labels.j2: {e:#}"));
    let get = |name: &str| {
        t.state()
            .lookup(name)
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| panic!("labels.j2: no {name}"))
    };
    Labels {
        unknown_counterparty: get("unknown_counterparty"),
        unknown_counterparty_means: get("unknown_counterparty_means"),
        no_category: get("no_category"),
        your_account: get("your_account"),
        owner: get("owner"),
        none_of_these: get("none_of_these"),
        no_counterparty_listed: get("no_counterparty_listed"),
        no_account_listed: get("no_account_listed"),
        cannot_tell: get("cannot_tell"),
        cannot_tell_means: get("cannot_tell_means"),
    }
});
