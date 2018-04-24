//! Module containing ways to represent a substitution.
//!
//! @file substitution.rs
//! @author Thibaut Schaeffer <thibaut@schaeff.fr>
//! @date 2018

const BINARY_SEPARATOR: &str = "_b";

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Substitution {
    hashmap: HashMap<String, String>
}

impl Substitution {
    pub fn new() -> Substitution {
        Substitution {
            hashmap: {
                HashMap::<String, String>::new()
            }
        }
    }

    pub fn insert(&mut self, key: String, element: String) -> Option<String>
    {
        let (p, _) = Self::split_key(&key);
        self.hashmap.insert(p.to_string(), element)
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let (p, s) = Self::split_key(key);
        match self.hashmap.get(p) {
            Some(ref v) => {
                match s {
                    Some(suffix) => {
                        Some(format!("{}{}{}", v, BINARY_SEPARATOR, suffix))
                    },
                    None => Some(v.to_string()),
                }
            },
            None => None
        }
    }

    pub fn contains_key(&mut self, key: &str) -> bool {
        let (p, _) = Self::split_key(&key);
        self.hashmap.contains_key(p)
    }

    fn split_key(key: &str) -> (&str, Option<&str>) {
        let mut parts = key.split(BINARY_SEPARATOR);
        (parts.nth(0).unwrap(), parts.nth(0))
    }
}