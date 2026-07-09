/// kebab-case a title exactly like hyper-frames `kebabSlug`.
///
/// Byte-compatible with `tools/studio/server/lib/util.ts`: lowercase, strip
/// everything outside `[a-z0-9]`/whitespace/`-`/`_`, collapse runs of
/// whitespace and `_`/`-` into a single `-`, then trim leading/trailing `-`.
/// Whitespace is Unicode-aware (`char::is_whitespace`) to match the JS `\s`.
pub fn kebab_slug(input: &str) -> String {
    let lowered = input.to_lowercase();
    let kept: String = lowered
        .chars()
        .filter(|c| {
            c.is_ascii_lowercase()
                || c.is_ascii_digit()
                || *c == '-'
                || *c == '_'
                || c.is_whitespace()
        })
        .collect();
    let mut out = String::with_capacity(kept.len());
    let mut prev_dash = false;
    for c in kept.chars() {
        let dash = c.is_whitespace() || c == '_' || c == '-';
        if dash {
            if !prev_dash {
                out.push('-');
            }
            prev_dash = true;
        } else {
            out.push(c);
            prev_dash = false;
        }
    }
    out.trim_matches('-').to_string()
}

/// Build a project slug: `{base}-{date}`, deduped to `{base}-{n}-{date}` from
/// `n = 2` on collision. An empty kebab base falls back to `"idea"`, matching
/// the studio promote flow (`server/routes/ideas.ts`).
pub fn project_slug(title: &str, date_iso: &str, exists: impl Fn(&str) -> bool) -> String {
    let base = {
        let b = kebab_slug(title);
        if b.is_empty() { "idea".to_string() } else { b }
    };
    let mut slug = format!("{base}-{date_iso}");
    let mut n = 2;
    while exists(&slug) {
        slug = format!("{base}-{n}-{date_iso}");
        n += 1;
    }
    slug
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("Hello World", "hello-world")]
    #[case("NVMe: Deep Dive!", "nvme-deep-dive")]
    #[case("a_b c", "a-b-c")]
    #[case("a - _ b", "a-b")]
    #[case("  spaced  ", "spaced")]
    #[case("--edge--", "edge")]
    #[case("Émigré café", "migr-caf")] // non-ascii stripped, exactly like the TS regex
    #[case("🔥🔥", "")]
    #[case("", "")]
    #[case("a\u{00A0}b", "a-b")] // NBSP is Unicode whitespace, collapses to a dash
    fn kebab_matches_studio_source(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(kebab_slug(input), expected);
    }

    #[test]
    fn empty_base_falls_back_to_idea() {
        assert_eq!(
            project_slug("🔥", "2026-07-09", |_| false),
            "idea-2026-07-09"
        );
    }

    #[test]
    fn collision_ladder_inserts_n_before_date() {
        let taken = ["nvme-deep-dive-2026-07-09", "nvme-deep-dive-2-2026-07-09"];
        let slug = project_slug("NVMe Deep Dive", "2026-07-09", |s| taken.contains(&s));
        assert_eq!(slug, "nvme-deep-dive-3-2026-07-09");
    }
}
