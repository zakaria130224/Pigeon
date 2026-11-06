//! The point-and-click checks on the Tests tab.

use super::{ResponseData, Vars};
use crate::model::*;

/// Look up a simple JSON path like `data.items[0].id`.
pub fn json_path<'a>(v: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut cur = v;
    let path = path.trim().trim_start_matches("$.").trim_start_matches('$');
    for part in path.split('.').filter(|p| !p.is_empty()) {
        let (name, idx) = match part.find('[') {
            Some(i) => (&part[..i], Some(&part[i..])),
            None => (part, None),
        };
        if !name.is_empty() {
            cur = cur.get(name)?;
        }
        if let Some(mut idxs) = idx {
            while let Some(s) = idxs.strip_prefix('[') {
                let end = s.find(']')?;
                let n: usize = s[..end].trim().parse().ok()?;
                cur = cur.get(n)?;
                idxs = &s[end + 1..];
            }
        }
    }
    Some(cur)
}

pub fn run_checks(checks: &[Check], r: &ResponseData, vars: &Vars) -> Vec<(String, bool, String)> {
    let json: Option<serde_json::Value> = serde_json::from_str(&r.body).ok();
    checks
        .iter()
        .filter(|c| c.enabled)
        .map(|c| {
            let exp = vars.resolve(c.expected.trim());
            match c.kind {
                CheckKind::StatusEquals => {
                    let ok = exp.parse::<u16>().map(|e| e == r.status).unwrap_or(false);
                    (format!("Status code is {}", exp), ok, format!("got {}", r.status))
                }
                CheckKind::BodyContains => {
                    let ok = r.body.contains(&exp);
                    (format!("Body contains \"{}\"", exp), ok, if ok { String::new() } else { "text not found".into() })
                }
                CheckKind::TimeBelow => {
                    let lim = exp.parse::<u64>().unwrap_or(0);
                    (format!("Response time is below {} ms", lim), r.time_ms < lim, format!("took {} ms", r.time_ms))
                }
                CheckKind::HeaderExists => {
                    let name = if c.target.is_empty() { exp.clone() } else { vars.resolve(&c.target) };
                    let ok = r.header(&name).is_some();
                    (format!("Header \"{}\" is present", name), ok, String::new())
                }
                CheckKind::JsonEquals => {
                    let got = json.as_ref().and_then(|j| json_path(j, &c.target));
                    let got_s = got
                        .map(|v| match v {
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                        .unwrap_or_else(|| "(missing)".into());
                    (format!("{} equals {}", c.target, exp), got_s == exp, format!("got {}", got_s))
                }
            }
        })
        .collect()
}
