use crate::core_ext::{Indent, Indexes};
use crate::{ArraySortingMode, CompareMode, Config, FloatCompareMode, NumericMode};
use float_cmp::{ApproxEq, F64Margin};
use serde_json::Value;
use std::{collections::HashSet, fmt};

pub(crate) fn diff<'a>(
    lhs: &'a Value,
    rhs: &'a Value,
    config: &'a Config,
) -> Vec<DifferenceRef<'a>> {
    let mut acc = vec![];
    diff_with(lhs, rhs, config, PathRef::Root, &mut acc);
    acc
}

fn diff_with<'a>(
    lhs: &'a Value,
    rhs: &'a Value,
    config: &'a Config,
    path: PathRef<'a>,
    acc: &mut Vec<DifferenceRef<'a>>,
) {
    let mut folder = DiffFolder {
        rhs,
        path,
        acc,
        config,
    };

    fold_json(lhs, &mut folder);
}

#[derive(Debug)]
struct DiffFolder<'a, 'b> {
    rhs: &'a Value,
    path: PathRef<'a>,
    acc: &'b mut Vec<DifferenceRef<'a>>,
    config: &'a Config,
}

macro_rules! direct_compare {
    ($name:ident) => {
        fn $name(&mut self, lhs: &'a Value) {
            if self.rhs != lhs {
                self.acc.push(DifferenceRef {
                    lhs: Some(lhs),
                    rhs: Some(&self.rhs),
                    path: self.path.clone(),
                    config: self.config.clone(),
                });
            }
        }
    };
}

impl<'a> DiffFolder<'a, '_> {
    direct_compare!(on_null);
    direct_compare!(on_bool);
    direct_compare!(on_string);

    fn on_number(&mut self, lhs: &'a Value) {
        let is_equal = match self.config.numeric_mode {
            NumericMode::Strict => self.eq_values(lhs, self.rhs),
            NumericMode::AssumeFloat => match (lhs.as_f64(), self.rhs.as_f64()) {
                (Some(lhs), Some(rhs)) => self.eq_floats(lhs, rhs),
                (lhs, rhs) => lhs == rhs,
            },
        };
        if !is_equal {
            self.acc.push(DifferenceRef {
                lhs: Some(lhs),
                rhs: Some(self.rhs),
                path: self.path.clone(),
                config: self.config.clone(),
            });
        }
    }

    fn eq_values(&self, lhs: &Value, rhs: &Value) -> bool {
        if lhs.is_f64() && rhs.is_f64() {
            // `as_f64` must return a floating point value if `is_f64` returned true. The inverse
            // relation is not guaranteed by serde_json.
            self.eq_floats(
                lhs.as_f64().expect("float value"),
                rhs.as_f64().expect("float value"),
            )
        } else {
            lhs == rhs
        }
    }

    fn eq_floats(&self, lhs: f64, rhs: f64) -> bool {
        if let FloatCompareMode::Epsilon(epsilon) = self.config.float_compare_mode {
            lhs.approx_eq(rhs, F64Margin::default().epsilon(epsilon))
        } else {
            lhs == rhs
        }
    }
    fn on_array_contains(&mut self, lhs: &'a Value) {
        if let Some(rhs) = self.rhs.as_array() {
            let lhs_array = lhs.as_array().unwrap();

            let lhs_len = lhs_array.len();
            let rhs_len = rhs.len();

            if self.config.compare_mode == CompareMode::Strict && lhs_len != rhs_len {
                self.acc.push(DifferenceRef {
                    lhs: Some(lhs),
                    rhs: Some(self.rhs),
                    path: self.path.clone(),
                    config: self.config.clone(),
                });
                return;
            }

            for rhs_item in rhs.iter() {
                // For each rhs item (expected) count the number of times it matches with the rhs
                // (expected) array.
                let rhs_item_count = rhs
                    .iter()
                    .filter(|i| diff(rhs_item, i, self.config).is_empty())
                    .count();
                // Make sure that lhs (actual) has at least as many items matching the rhs
                // (expected) item.
                let lhs_matching_items_count = lhs_array
                    .iter()
                    .filter(|lhs_item| diff(lhs_item, rhs_item, self.config).is_empty())
                    .count();
                if lhs_matching_items_count < rhs_item_count {
                    self.acc.push(DifferenceRef {
                        lhs: Some(lhs),
                        rhs: Some(self.rhs),
                        path: self.path.clone(),
                        config: self.config.clone(),
                    });
                    break;
                }
            }
        } else {
            self.acc.push(DifferenceRef {
                lhs: Some(lhs),
                rhs: Some(self.rhs),
                path: self.path.clone(),
                config: self.config.clone(),
            });
        }
    }

    fn on_array(&mut self, lhs: &'a Value) {
        if self.config.array_sorting_mode == ArraySortingMode::Ignore {
            return self.on_array_contains(lhs);
        }

        if let Some(rhs) = self.rhs.as_array() {
            let lhs = lhs.as_array().unwrap();

            match self.config.compare_mode {
                CompareMode::Inclusive => {
                    for (idx, rhs) in rhs.iter().enumerate() {
                        let path = self.path.append(KeyRef::Idx(idx));

                        if let Some(lhs) = lhs.get(idx) {
                            diff_with(lhs, rhs, self.config, path, self.acc)
                        } else {
                            self.acc.push(DifferenceRef {
                                lhs: None,
                                rhs: Some(self.rhs),
                                path,
                                config: self.config.clone(),
                            });
                        }
                    }
                }
                CompareMode::Strict => {
                    let all_keys = rhs
                        .indexes()
                        .into_iter()
                        .chain(lhs.indexes())
                        .collect::<HashSet<_>>();
                    for key in all_keys {
                        let path = self.path.append(KeyRef::Idx(key));

                        match (lhs.get(key), rhs.get(key)) {
                            (Some(lhs), Some(rhs)) => {
                                diff_with(lhs, rhs, self.config, path, self.acc);
                            }
                            (None, Some(rhs)) => {
                                self.acc.push(DifferenceRef {
                                    lhs: None,
                                    rhs: Some(rhs),
                                    path,
                                    config: self.config.clone(),
                                });
                            }
                            (Some(lhs), None) => {
                                self.acc.push(DifferenceRef {
                                    lhs: Some(lhs),
                                    rhs: None,
                                    path,
                                    config: self.config.clone(),
                                });
                            }
                            (None, None) => {
                                unreachable!("at least one of the maps should have the key")
                            }
                        }
                    }
                }
            }
        } else {
            self.acc.push(DifferenceRef {
                lhs: Some(lhs),
                rhs: Some(self.rhs),
                path: self.path.clone(),
                config: self.config.clone(),
            });
        }
    }

    fn on_object(&mut self, lhs: &'a Value) {
        if let Some(rhs) = self.rhs.as_object() {
            let lhs = lhs.as_object().unwrap();

            match self.config.compare_mode {
                CompareMode::Inclusive => {
                    for (key, rhs) in rhs.iter() {
                        let path = self.path.append(KeyRef::Field(key));

                        if let Some(lhs) = lhs.get(key) {
                            diff_with(lhs, rhs, self.config, path, self.acc)
                        } else {
                            self.acc.push(DifferenceRef {
                                lhs: None,
                                rhs: Some(self.rhs),
                                path,
                                config: self.config.clone(),
                            });
                        }
                    }
                }
                CompareMode::Strict => {
                    let all_keys = rhs.keys().chain(lhs.keys()).collect::<HashSet<_>>();
                    for key in all_keys {
                        let path = self.path.append(KeyRef::Field(key));

                        match (lhs.get(key), rhs.get(key)) {
                            (Some(lhs), Some(rhs)) => {
                                diff_with(lhs, rhs, self.config, path, self.acc);
                            }
                            (None, Some(rhs)) => {
                                self.acc.push(DifferenceRef {
                                    lhs: None,
                                    rhs: Some(rhs),
                                    path,
                                    config: self.config.clone(),
                                });
                            }
                            (Some(lhs), None) => {
                                self.acc.push(DifferenceRef {
                                    lhs: Some(lhs),
                                    rhs: None,
                                    path,
                                    config: self.config.clone(),
                                });
                            }
                            (None, None) => {
                                unreachable!("at least one of the maps should have the key")
                            }
                        }
                    }
                }
            }
        } else {
            self.acc.push(DifferenceRef {
                lhs: Some(lhs),
                rhs: Some(self.rhs),
                path: self.path.clone(),
                config: self.config.clone(),
            });
        }
    }
}

/// Represents a difference between two JSON values.
#[derive(Debug, PartialEq, Clone)]
pub struct Difference {
    path: Path,
    lhs: Option<Value>,
    rhs: Option<Value>,
    config: Config,
}

impl<'a> From<DifferenceRef<'a>> for Difference {
    fn from(diff: DifferenceRef<'a>) -> Self {
        Difference {
            path: Path::from(diff.path),
            lhs: diff.lhs.cloned(),
            rhs: diff.rhs.cloned(),
            config: diff.config.clone(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct DifferenceRef<'a> {
    path: PathRef<'a>,
    lhs: Option<&'a Value>,
    rhs: Option<&'a Value>,
    config: Config,
}

impl fmt::Display for DifferenceRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let json_to_string = |json: &Value| serde_json::to_string_pretty(json).unwrap();

        match (&self.config.compare_mode, &self.lhs, &self.rhs) {
            (CompareMode::Inclusive, Some(actual), Some(expected)) => {
                writeln!(f, "json atoms at path \"{}\" are not equal:", self.path)?;
                writeln!(f, "    expected:")?;
                writeln!(f, "{}", json_to_string(expected).indent(8))?;
                writeln!(f, "    actual:")?;
                write!(f, "{}", json_to_string(actual).indent(8))?;
            }
            (CompareMode::Inclusive, None, Some(_expected)) => {
                write!(
                    f,
                    "json atom at path \"{}\" is missing from actual",
                    self.path
                )?;
            }
            (CompareMode::Inclusive, Some(_actual), None) => {
                unreachable!("stuff missing actual wont produce an error")
            }
            (CompareMode::Inclusive, None, None) => unreachable!("can't both be missing"),

            (CompareMode::Strict, Some(lhs), Some(rhs)) => {
                writeln!(f, "json atoms at path \"{}\" are not equal:", self.path)?;
                writeln!(f, "    lhs:")?;
                writeln!(f, "{}", json_to_string(lhs).indent(8))?;
                writeln!(f, "    rhs:")?;
                write!(f, "{}", json_to_string(rhs).indent(8))?;
            }
            (CompareMode::Strict, None, Some(_)) => {
                write!(f, "json atom at path \"{}\" is missing from lhs", self.path)?;
            }
            (CompareMode::Strict, Some(_), None) => {
                write!(f, "json atom at path \"{}\" is missing from rhs", self.path)?;
            }
            (CompareMode::Strict, None, None) => unreachable!("can't both be missing"),
        }

        Ok(())
    }
}

/// Represents a path to a JSON value in a tree structure.
#[derive(Debug, Clone, PartialEq)]
enum Path {
    Root,
    Keys(Vec<Key>),
}

impl<'a> From<PathRef<'a>> for Path {
    fn from(path: PathRef<'a>) -> Self {
        match path {
            PathRef::Root => Path::Root,
            PathRef::Keys(keys) => Path::Keys(keys.into_iter().map(Key::from).collect()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum PathRef<'a> {
    Root,
    Keys(Vec<KeyRef<'a>>),
}

impl<'a> PathRef<'a> {
    fn append(&self, next: KeyRef<'a>) -> PathRef<'a> {
        match self {
            PathRef::Root => PathRef::Keys(vec![next]),
            PathRef::Keys(list) => {
                let mut copy = list.clone();
                copy.push(next);
                PathRef::Keys(copy)
            }
        }
    }
}

impl fmt::Display for PathRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PathRef::Root => write!(f, "(root)"),
            PathRef::Keys(keys) => {
                for key in keys {
                    write!(f, "{}", key)?;
                }
                Ok(())
            }
        }
    }
}

/// Represents a key in a JSON object or an index in a JSON array.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Key {
    Idx(usize),
    Field(String),
}

impl<'a> From<KeyRef<'a>> for Key {
    fn from(key: KeyRef<'a>) -> Self {
        match key {
            KeyRef::Idx(idx) => Key::Idx(idx),
            KeyRef::Field(field) => Key::Field(field.to_owned()),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum KeyRef<'a> {
    Idx(usize),
    Field(&'a str),
}

impl fmt::Display for KeyRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            KeyRef::Idx(idx) => write!(f, "[{}]", idx),
            KeyRef::Field(key) => write!(f, ".{}", key),
        }
    }
}

fn fold_json<'a>(json: &'a Value, folder: &mut DiffFolder<'a, '_>) {
    match json {
        Value::Null => folder.on_null(json),
        Value::Bool(_) => folder.on_bool(json),
        Value::Number(_) => folder.on_number(json),
        Value::String(_) => folder.on_string(json),
        Value::Array(_) => folder.on_array(json),
        Value::Object(_) => folder.on_object(json),
    }
}

#[cfg(test)]
mod test {
    #[allow(unused_imports)]
    use super::*;
    use serde_json::json;

    #[test]
    fn test_diffing_leaf_json() {
        let config = Config::new(CompareMode::Inclusive);
        let diffs = diff(&json!(null), &json!(null), &config);
        assert_eq!(diffs, vec![]);

        let diffs = diff(&json!(false), &json!(false), &config);
        assert_eq!(diffs, vec![]);

        let diffs = diff(&json!(true), &json!(true), &config);
        assert_eq!(diffs, vec![]);

        let diffs = diff(&json!(false), &json!(true), &config);
        assert_eq!(diffs.len(), 1);

        let diffs = diff(&json!(true), &json!(false), &config);
        assert_eq!(diffs.len(), 1);

        let actual = json!(1);
        let expected = json!(1);
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs, vec![]);

        let actual = json!(2);
        let expected = json!(1);
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs.len(), 1);

        let actual = json!(1);
        let expected = json!(2);
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs.len(), 1);

        let actual = json!(1.0);
        let expected = json!(1.0);
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs, vec![]);

        let actual = json!(1);
        let expected = json!(1.0);
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs.len(), 1);

        let actual = json!(1.0);
        let expected = json!(1);
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs.len(), 1);

        let config_assume_float = config.numeric_mode(NumericMode::AssumeFloat);

        let actual = json!(1);
        let expected = json!(1.0);
        let diffs = diff(&actual, &expected, &config_assume_float);
        assert_eq!(diffs, vec![]);

        let actual = json!(1.0);
        let expected = json!(1);
        let diffs = diff(&actual, &expected, &config_assume_float);
        assert_eq!(diffs, vec![]);

        let actual = json!(1.15);
        let expected = json!(1);
        let config = Config::new(CompareMode::Inclusive)
            .numeric_mode(NumericMode::AssumeFloat)
            .float_compare_mode(FloatCompareMode::Epsilon(0.2));
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs, vec![]);

        let actual = json!(1.25);
        let expected = json!(1);
        let config = Config::new(CompareMode::Inclusive)
            .numeric_mode(NumericMode::AssumeFloat)
            .float_compare_mode(FloatCompareMode::Epsilon(0.2));

        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs.len(), 1);

        let actual = json!(2);
        let expected = json!(1);
        let config =
            Config::new(CompareMode::Inclusive).float_compare_mode(FloatCompareMode::Epsilon(2.0));

        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs.len(), 1);
    }

    #[test]
    fn test_diffing_array() {
        let config = Config::new(CompareMode::Inclusive);
        // empty
        let actual = json!([]);
        let expected = json!([]);
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs, vec![]);

        let actual = json!([1]);
        let expected = json!([]);
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs.len(), 0);

        let actual = json!([]);
        let expected = json!([1]);
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs.len(), 1);

        // eq
        let actual = json!([1]);
        let expected = json!([1]);
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs, vec![]);

        // actual longer
        let actual = json!([1, 2]);
        let expected = json!([1]);
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs, vec![]);

        // expected longer
        let actual = json!([1]);
        let expected = json!([1, 2]);
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs.len(), 1);

        // eq length but different
        let actual = json!([1, 3]);
        let expected = json!([1, 2]);
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs.len(), 1);

        // different types
        let actual = json!(1);
        let expected = json!([1]);
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs.len(), 1);

        let actual = json!([1]);
        let expected = json!(1);
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs.len(), 1);
    }

    #[test]
    fn test_array_strict() {
        let config = Config::new(CompareMode::Strict);
        let actual = json!([]);
        let expected = json!([]);
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs.len(), 0);

        let actual = json!([1, 2]);
        let expected = json!([1, 2]);
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs.len(), 0);

        let actual = json!([1]);
        let expected = json!([1, 2]);
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs.len(), 1);

        let actual = json!([1, 2]);
        let expected = json!([1]);
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs.len(), 1);
    }

    #[test]
    fn test_object() {
        let config = Config::new(CompareMode::Inclusive);
        let actual = json!({});
        let expected = json!({});
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs, vec![]);

        let actual = json!({ "a": 1 });
        let expected = json!({ "a": 1 });
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs, vec![]);

        let actual = json!({ "a": 1, "b": 123 });
        let expected = json!({ "a": 1 });
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs, vec![]);

        let actual = json!({ "a": 1 });
        let expected = json!({ "b": 1 });
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs.len(), 1);

        let actual = json!({ "a": 1 });
        let expected = json!({ "a": 2 });
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs.len(), 1);

        let actual = json!({ "a": { "b": true } });
        let expected = json!({ "a": {} });
        let diffs = diff(&actual, &expected, &config);
        assert_eq!(diffs, vec![]);
    }

    #[test]
    fn test_object_strict() {
        let config = Config::new(CompareMode::Strict);
        let lhs = json!({});
        let rhs = json!({ "a": 1 });
        let diffs = diff(&lhs, &rhs, &config);
        assert_eq!(diffs.len(), 1);

        let lhs = json!({ "a": 1 });
        let rhs = json!({});
        let diffs = diff(&lhs, &rhs, &config);
        assert_eq!(diffs.len(), 1);

        let json = json!({ "a": 1 });
        let diffs = diff(&json, &json, &config);
        assert_eq!(diffs, vec![]);
    }
}
