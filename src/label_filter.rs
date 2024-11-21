use std::{collections::BTreeMap, str::FromStr};

use crate::error::RobotLBError;

/// Enum of all possible rules for label filtering.
#[derive(Debug, Clone)]
enum Rule {
    /// Equal rule checks if the key is equal to the value.
    Equal(String, String),
    /// `NotEqual` rule checks if the key is not equal to the value.
    NotEqual(String, String),
    /// Exists rule checks if the key exists.
    Exists(String),
    /// `DoesNotExist` rule checks if the key does not exist.
    DoesNotExist(String),
}

/// `LabelFilter` is a filter for Kubernetes labels.
/// It is used to filter nodes by their labels.
#[derive(Debug, Clone, Default)]
pub struct LabelFilter {
    rules: Vec<Rule>,
}

impl LabelFilter {
    #[must_use]
    pub fn check(&self, labels: &BTreeMap<String, String>) -> bool {
        for rule in &self.rules {
            match rule {
                Rule::Equal(key, value) => {
                    if labels.get(key) != Some(value) {
                        return false;
                    }
                }
                Rule::NotEqual(key, value) => {
                    if labels.get(key) == Some(value) {
                        return false;
                    }
                }
                Rule::Exists(key) => {
                    if labels.get(key).is_none() {
                        return false;
                    }
                }
                Rule::DoesNotExist(key) => {
                    if labels.get(key).is_some() {
                        return false;
                    }
                }
            }
        }
        true
    }
}

/// Parse label filter from string.
/// The string should be in the following format:
/// `key=value,key!=value,key,!key`
impl FromStr for LabelFilter {
    type Err = RobotLBError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut rules = Vec::new();
        for rule in s.split(',') {
            let parts = rule.split('=').collect::<Vec<_>>();
            match *parts.as_slice() {
                [key] => {
                    if key.starts_with('!') {
                        rules.push(Rule::DoesNotExist(
                            key.strip_prefix('!').unwrap().to_string(),
                        ));
                        continue;
                    }
                    rules.push(Rule::Exists(key.to_string()));
                }
                [key, value] => {
                    if key.ends_with('!') {
                        rules.push(Rule::NotEqual(
                            key.strip_suffix('!').unwrap().to_string(),
                            value.to_string(),
                        ));
                        continue;
                    }
                    rules.push(Rule::Equal(key.to_string(), value.to_string()));
                }
                _ => return Err(RobotLBError::InvalidNodeFilter(rule.to_string())),
            }
        }
        Ok(Self { rules })
    }
}
