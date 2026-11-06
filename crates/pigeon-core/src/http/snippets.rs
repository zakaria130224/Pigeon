//! Code snippets for the `</>` panel.

use super::Prepared;

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn curl_snippet(p: &Prepared) -> String {
    let mut s = format!("curl --location --request {} \\\n  {}", p.method, shell_quote(&p.url));
    for (k, v) in &p.headers {
        s.push_str(&format!(" \\\n  --header {}", shell_quote(&format!("{}: {}", k, v))));
    }
    if let Some(b) = &p.body
        && !b.is_empty()
    {
        s.push_str(&format!(" \\\n  --data-raw {}", shell_quote(&String::from_utf8_lossy(b))));
    }
    s
}

pub fn fetch_snippet(p: &Prepared) -> String {
    let mut h = String::new();
    for (k, v) in &p.headers {
        h.push_str(&format!("    {:?}: {:?},\n", k, v));
    }
    let body =
        p.body.as_ref().filter(|b| !b.is_empty()).map(|b| format!("  body: {:?},\n", String::from_utf8_lossy(b))).unwrap_or_default();
    format!(
        "const response = await fetch({:?}, {{\n  method: {:?},\n  headers: {{\n{}  }},\n{}}});\nconsole.log(await response.text());",
        p.url, p.method, h, body
    )
}

pub fn python_snippet(p: &Prepared) -> String {
    let mut h = String::new();
    for (k, v) in &p.headers {
        h.push_str(&format!("    {:?}: {:?},\n", k, v));
    }
    let body = p
        .body
        .as_ref()
        .filter(|b| !b.is_empty())
        .map(|b| format!("payload = {:?}\n", String::from_utf8_lossy(b)))
        .unwrap_or_else(|| "payload = None\n".into());
    format!(
        "import requests\n\nurl = {:?}\n{}headers = {{\n{}}}\n\nresponse = requests.request({:?}, url, headers=headers, data=payload)\nprint(response.text)",
        p.url, body, h, p.method
    )
}
