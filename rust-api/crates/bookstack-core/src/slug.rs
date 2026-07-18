/// Generate a URL slug from a display name, BookStack style.
pub fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_dash = true; // avoid leading dash
    for ch in name.chars() {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                out.push(lower);
            }
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out.truncate(180);
    if out.is_empty() {
        "item".to_string()
    } else {
        out
    }
}

/// Produce a deduplicated slug given a probe callback reporting whether a
/// candidate is already taken.
pub async fn unique_slug<F, Fut>(base: &str, taken: F) -> crate::Result<String>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = crate::Result<bool>>,
{
    let base = slugify(base);
    if !taken(base.clone()).await? {
        return Ok(base);
    }
    for n in 2..100 {
        let candidate = format!("{base}-{n}");
        if !taken(candidate.clone()).await? {
            return Ok(candidate);
        }
    }
    // Practically unreachable; fall back to a random suffix.
    let suffix: u32 = rand::random();
    Ok(format!("{base}-{suffix:x}"))
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn basic() {
        assert_eq!(slugify("My Great Page"), "my-great-page");
        assert_eq!(slugify("  Hello -- World!  "), "hello-world");
        assert_eq!(slugify("Ünïcode Ünïcode"), "ünïcode-ünïcode");
        assert_eq!(slugify("!!!"), "item");
        assert_eq!(slugify(""), "item");
    }
}
