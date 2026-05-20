#[allow(dead_code)]
use super::card::Card;

/// Extract unique labels from a list of cards.
pub fn extract_labels(cards: &[Card]) -> Vec<String> {
    let mut labels = std::collections::HashSet::new();
    for card in cards {
        for label in &card.labels {
            labels.insert(label.clone());
        }
    }
    let mut result: Vec<String> = labels.into_iter().collect();
    result.sort();
    result
}

/// Check if any card has the given label.
pub fn cards_with_label<'a>(cards: &'a [Card], label: &str) -> Vec<&'a Card> {
    cards.iter().filter(|c| c.labels.iter().any(|l| l == label)).collect()
}

/// Filter cards by a list of labels (cards must have ALL specified labels).
pub fn filter_by_labels<'a>(cards: &'a [Card], labels: &[String]) -> Vec<&'a Card> {
    if labels.is_empty() {
        return cards.iter().collect();
    }
    cards.iter().filter(|c| {
        labels.iter().all(|l| c.labels.contains(l))
    }).collect()
}
