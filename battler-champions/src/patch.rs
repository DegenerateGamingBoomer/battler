//! JSON patching for resource data.
//!
//! A patch is an RFC 7396 style merge patch with two list extensions, so that patches do not have
//! to restate whole lists (such as move flags):
//! - `"+key": [..]` appends the values that are not already in the list at `key`.
//! - `"-key": [..]` removes the given values from the list at `key`.
//!
//! Plain keys replace scalars and lists, merge objects recursively, and delete the key for `null`.

use serde_json::{
    Map,
    Value,
};

/// Applies `patch` to `base`.
pub fn apply_patch(base: &mut Value, patch: &Value) {
    let Value::Object(patch) = patch else {
        *base = patch.clone();
        return;
    };
    if !base.is_object() {
        *base = Value::Object(Map::new());
    }
    let base = base.as_object_mut().unwrap();
    // Apply plain keys before list edits, so a patch can replace a list and edit it.
    for (key, value) in patch.iter().filter(|(k, _)| !is_list_edit(k)) {
        if value.is_null() {
            base.remove(key);
        } else if value.is_object() && base.get(key).is_some_and(Value::is_object) {
            apply_patch(base.get_mut(key).unwrap(), value);
        } else {
            let mut cleaned = value.clone();
            strip_nulls(&mut cleaned);
            base.insert(key.clone(), cleaned);
        }
    }
    for (key, value) in patch.iter().filter(|(k, _)| is_list_edit(k)) {
        let (add, name) = (key.starts_with('+'), &key[1..]);
        let list = base
            .entry(name.to_owned())
            .or_insert_with(|| Value::Array(Vec::new()));
        let Some(list) = list.as_array_mut() else {
            continue;
        };
        for item in value.as_array().into_iter().flatten() {
            if add {
                if !list.contains(item) {
                    list.push(item.clone());
                }
            } else {
                list.retain(|v| v != item);
            }
        }
    }
}

fn is_list_edit(key: &str) -> bool {
    key.len() > 1 && (key.starts_with('+') || key.starts_with('-'))
}

fn strip_nulls(value: &mut Value) {
    if let Value::Object(map) = value {
        map.retain(|_, v| !v.is_null());
        map.values_mut().for_each(strip_nulls);
    }
}

#[cfg(test)]
mod patch_test {
    use serde_json::json;

    use super::apply_patch;

    #[test]
    fn replaces_and_merges() {
        let mut base = json!({"pp": 10, "effect": {"callbacks": {"a": 1, "b": 2}}, "flags": ["X"]});
        apply_patch(
            &mut base,
            &json!({"pp": 5, "effect": {"callbacks": {"a": null, "c": 3}}}),
        );
        assert_eq!(
            base,
            json!({"pp": 5, "effect": {"callbacks": {"b": 2, "c": 3}}, "flags": ["X"]})
        );
    }

    #[test]
    fn edits_lists() {
        let mut base = json!({"flags": ["A", "B"]});
        apply_patch(&mut base, &json!({"+flags": ["B", "C"], "-flags": ["A"]}));
        assert_eq!(base, json!({"flags": ["B", "C"]}));
        let mut base = json!({});
        apply_patch(&mut base, &json!({"+flags": ["A"]}));
        assert_eq!(base, json!({"flags": ["A"]}));
    }

    #[test]
    fn replaces_lists() {
        let mut base = json!({"secondary_effects": [{"chance": "30%"}]});
        apply_patch(&mut base, &json!({"secondary_effects": []}));
        assert_eq!(base, json!({"secondary_effects": []}));
    }
}
