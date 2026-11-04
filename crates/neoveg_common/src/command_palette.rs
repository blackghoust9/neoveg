//! The search of the command palette (*Exibir > Paleta de comandos*): a few words typed, the
//! commands whose name holds all of them, the best match first. It is an addition to the menus and
//! the keyboard map of the VEGAS, which stay as they are; the palette only finds and runs what they
//! already have.
//!
//! The text is compared without case and without accents, in any language the interface has (the
//! caller puts the translated name, the Portuguese name and the id of a command in one haystack).

/// `text` in lower case, with the accents of the Latin letters taken off.
pub fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            _ => c,
        })
        .collect()
}

/// Whether `index` is the start of a word of `text` (the start, or after a space, dot, dash or
/// underscore).
fn word_start(text: &str, index: usize) -> bool {
    index == 0
        || text[..index]
            .chars()
            .next_back()
            .is_some_and(|c| !c.is_alphanumeric())
}

/// How well `haystack` (already folded) answers `words` (already folded): lower is better; `None`
/// when a word is missing. A word at the start of a word of the haystack, and early in it, wins.
pub fn score(words: &[&str], haystack: &str) -> Option<usize> {
    let mut total = 0;
    for word in words {
        let found = haystack
            .match_indices(word)
            .map(|(i, _)| i + if word_start(haystack, i) { 0 } else { 40 })
            .min()?;
        total += found;
    }
    Some(total + haystack.len() / 8)
}

/// The indices of `haystacks` that answer `query`, best first and at most `limit`. A query with no
/// words keeps every haystack in its own order.
pub fn search(query: &str, haystacks: &[String], limit: usize) -> Vec<usize> {
    let folded = fold(query);
    let words: Vec<&str> = folded.split_whitespace().collect();
    if words.is_empty() {
        return (0..haystacks.len().min(limit)).collect();
    }
    let mut hits: Vec<(usize, usize)> = haystacks
        .iter()
        .enumerate()
        .filter_map(|(i, h)| score(&words, &fold(h)).map(|s| (s, i)))
        .collect();
    hits.sort();
    hits.into_iter().take(limit).map(|(_, i)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn text_is_compared_without_case_or_accents() {
        assert_eq!(fold("Áudio Ção ÑANDÚ"), "audio cao nandu");
        let items = list(&["Inserir faixa de áudio", "Inserir faixa de vídeo"]);
        assert_eq!(search("AUDIO", &items, 10), vec![0]);
        assert_eq!(search("video", &items, 10), vec![1]);
    }

    #[test]
    fn every_word_has_to_be_there_in_any_order() {
        let items = list(&[
            "Inserir faixa de áudio",
            "Remover faixa",
            "Inserir marcador",
        ]);
        assert_eq!(search("faixa inserir", &items, 10), vec![0]);
        assert_eq!(search("faixa", &items, 10).len(), 2);
        assert!(search("faixa xyz", &items, 10).is_empty());
    }

    #[test]
    fn a_match_at_the_start_of_a_word_and_early_comes_first() {
        let items = list(&[
            "Ir para o início",
            "Mover o corte",
            "Cortar",
            "Desfazer o corte",
        ]);
        let found = search("cort", &items, 10);
        assert_eq!(found[0], 2, "the word that starts with it wins: {found:?}");
        assert_eq!(found.len(), 3);
        assert!(found.contains(&1) && found.contains(&3));
    }

    #[test]
    fn an_empty_query_lists_everything_in_order_and_the_limit_cuts_the_list() {
        let items = list(&["a", "b", "c"]);
        assert_eq!(search("  ", &items, 10), vec![0, 1, 2]);
        assert_eq!(search("", &items, 2), vec![0, 1]);
        assert_eq!(search("a b", &list(&["a b", "b a"]), 1).len(), 1);
        assert!(search("x", &[], 5).is_empty());
    }
}
