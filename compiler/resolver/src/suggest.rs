//! Finding the name the author probably meant.
//!
//! "cannot find variable `lenght`" is a fine diagnostic. "cannot find
//! variable `lenght`, did you mean `length`?" saves the reader from
//! re-reading their own code, which is the whole point of taking
//! diagnostics seriously.

/// The candidate closest to `name`, if one is close enough to be worth
/// suggesting.
///
/// The threshold scales with length: a one-character typo in a short name
/// is plausible, but two edits in a three-letter name means a different
/// word, and guessing wrong is worse than not guessing.
pub fn closest<'a>(name: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let budget = (name.chars().count() / 3).max(1);
    let mut best: Option<(usize, &str)> = None;
    for candidate in candidates {
        if candidate == name {
            continue;
        }
        let distance = edit_distance(name, candidate);
        if distance > budget {
            continue;
        }
        // Ties go to the first candidate seen, which keeps the suggestion
        // stable rather than dependent on hash ordering.
        if best.is_none_or(|(d, _)| distance < d) {
            best = Some((distance, candidate));
        }
    }
    best.map(|(_, c)| c)
}

/// Levenshtein distance, counting insertions, deletions and substitutions.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }

    // Only the previous row is needed at any point.
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitution = prev[j] + usize::from(ca != cb);
            let deletion = prev[j + 1] + 1;
            let insertion = current[j] + 1;
            current[j + 1] = substitution.min(deletion).min(insertion);
        }
        std::mem::swap(&mut prev, &mut current);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_counts_single_edits() {
        assert_eq!(edit_distance("", ""), 0);
        assert_eq!(edit_distance("abc", "abc"), 0);
        assert_eq!(edit_distance("abc", "abd"), 1); // substitution
        assert_eq!(edit_distance("abc", "ab"), 1); // deletion
        assert_eq!(edit_distance("abc", "abcd"), 1); // insertion
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("", "abc"), 3);
    }

    #[test]
    fn suggests_a_near_miss() {
        assert_eq!(closest("lenght", ["length", "width"]), Some("length"));
        assert_eq!(closest("Strng", ["String", "Int"]), Some("String"));
        assert_eq!(closest("prntln", ["println"]), Some("println"));
    }

    #[test]
    fn stays_quiet_when_nothing_is_close() {
        assert_eq!(closest("banana", ["length", "width"]), None);
        // Two edits in a short name is a different word.
        assert_eq!(closest("abc", ["xyz"]), None);
        assert_eq!(closest("foo", ["fob"]), Some("fob"));
    }

    #[test]
    fn never_suggests_the_name_itself() {
        assert_eq!(closest("length", ["length"]), None);
    }

    #[test]
    fn picks_the_closest_candidate() {
        assert_eq!(closest("cont", ["count", "constant"]), Some("count"));
    }
}
