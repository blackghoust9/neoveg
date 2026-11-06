//! Media Bins & Smart Search Bins (results.md §12.10; manual pp. 169–172).
//!
//! Provides the hierarchical virtual folder tree, immutable root "All Media",
//! 10 Quick Tags (Ctrl+1..Ctrl+0), auto-renaming on duplicate names, and
//! dynamic reactive Smart Search Bins.

use crate::media_pool::MediaDetailRow;
use neoveg_common::alerts::AlertCode;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub type BinId = u64;

/// Target metadata field for smart bin query conditions (Dialog 869 / 872).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchField {
    Name,
    Type,
    Comment,
    TapeName,
    Path,
    AudioFormat,
    FieldOrder,
    Tag,
}

/// Comparison condition for smart bin queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchCondition {
    Contains,
    DoesNotContain,
    StartsWith,
    EndsWith,
    Equals,
    NotEquals,
    IsEmpty,
    IsNotEmpty,
}

/// Predicate criterion for dynamic search in smart bins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchCriterion {
    pub field: SearchField,
    pub condition: SearchCondition,
    pub phrase: String,
}

impl SearchCriterion {
    pub fn new(field: SearchField, condition: SearchCondition, phrase: impl Into<String>) -> Self {
        Self {
            field,
            condition,
            phrase: phrase.into(),
        }
    }

    /// Evaluates this single criterion against a media detail row.
    pub fn matches(&self, item: &MediaDetailRow) -> bool {
        let text = match self.field {
            SearchField::Name => &item.name,
            SearchField::Type => &item.media_type,
            SearchField::Comment => &item.comment,
            SearchField::TapeName => &item.tape_name,
            SearchField::Path => item.path.to_str().unwrap_or(""),
            SearchField::AudioFormat => &item.audio_format,
            SearchField::FieldOrder => &item.field_order,
            SearchField::Tag => {
                let phrase_lower = self.phrase.to_lowercase();
                return match self.condition {
                    SearchCondition::Contains | SearchCondition::Equals => {
                        item.tags.iter().any(|t| {
                            t.to_lowercase() == phrase_lower
                                || t.to_lowercase().contains(&phrase_lower)
                        })
                    }
                    SearchCondition::DoesNotContain | SearchCondition::NotEquals => !item
                        .tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&phrase_lower)),
                    SearchCondition::IsEmpty => item.tags.is_empty(),
                    SearchCondition::IsNotEmpty => !item.tags.is_empty(),
                    _ => false,
                };
            }
        };

        let text_lower = text.to_lowercase();
        let phrase_lower = self.phrase.to_lowercase();

        match self.condition {
            SearchCondition::Contains => text_lower.contains(&phrase_lower),
            SearchCondition::DoesNotContain => !text_lower.contains(&phrase_lower),
            SearchCondition::StartsWith => text_lower.starts_with(&phrase_lower),
            SearchCondition::EndsWith => text_lower.ends_with(&phrase_lower),
            SearchCondition::Equals => text_lower == phrase_lower,
            SearchCondition::NotEquals => text_lower != phrase_lower,
            SearchCondition::IsEmpty => text.trim().is_empty(),
            SearchCondition::IsNotEmpty => !text.trim().is_empty(),
        }
    }
}

/// Virtual folder in the Project Media bin hierarchy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaBin {
    pub id: BinId,
    pub parent_id: Option<BinId>,
    pub name: String,
    pub is_root: bool,
    pub is_smart: bool,
    pub search_criteria: Vec<SearchCriterion>,
    pub media_ids: HashSet<u64>,
}

/// Hierarchical container managing media bins, quick tags (Ctrl+1..0), and smart queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaBinTree {
    root_id: BinId,
    tags_root_id: BinId,
    bins: HashMap<BinId, MediaBin>,
    quick_tags: [String; 10], // Shortcuts Ctrl+1 through Ctrl+0 (Dialog 871)
    next_id: u64,
}

impl Default for MediaBinTree {
    fn default() -> Self {
        Self::new()
    }
}

impl MediaBinTree {
    pub const ROOT_BIN_ID: BinId = 1;
    pub const TAGS_ROOT_BIN_ID: BinId = 2;

    pub fn new() -> Self {
        let mut bins = HashMap::new();

        // 1. Root "All Media" bin (immutable)
        bins.insert(
            Self::ROOT_BIN_ID,
            MediaBin {
                id: Self::ROOT_BIN_ID,
                parent_id: None,
                name: "All Media".into(),
                is_root: true,
                is_smart: false,
                search_criteria: Vec::new(),
                media_ids: HashSet::new(),
            },
        );

        // 2. Tags root container
        bins.insert(
            Self::TAGS_ROOT_BIN_ID,
            MediaBin {
                id: Self::TAGS_ROOT_BIN_ID,
                parent_id: Some(Self::ROOT_BIN_ID),
                name: "Tags".into(),
                is_root: false,
                is_smart: false,
                search_criteria: Vec::new(),
                media_ids: HashSet::new(),
            },
        );

        let default_quick_tags = [
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ];

        Self {
            root_id: Self::ROOT_BIN_ID,
            tags_root_id: Self::TAGS_ROOT_BIN_ID,
            bins,
            quick_tags: default_quick_tags,
            next_id: 3,
        }
    }

    pub fn root_id(&self) -> BinId {
        self.root_id
    }

    pub fn get_bin(&self, id: BinId) -> Option<&MediaBin> {
        self.bins.get(&id)
    }

    pub fn get_bin_mut(&mut self, id: BinId) -> Option<&mut MediaBin> {
        self.bins.get_mut(&id)
    }

    pub fn all_bins(&self) -> impl Iterator<Item = &MediaBin> {
        self.bins.values()
    }

    /// Creates a new user bin under `parent_id` (defaulting to root if None).
    ///
    /// Validates non-empty name (Alert ID 12887).
    /// Auto-renames duplicate names in same parent (Alert ID 18174).
    pub fn create_bin(
        &mut self,
        parent_id: Option<BinId>,
        requested_name: &str,
    ) -> Result<BinId, AlertCode> {
        let trimmed = requested_name.trim();
        if trimmed.is_empty() {
            return Err(AlertCode::BinNameEmpty);
        }

        let parent = parent_id.unwrap_or(self.root_id);
        let unique_name = self.resolve_unique_name(parent, trimmed, None);

        let id = self.next_id;
        self.next_id += 1;

        let bin = MediaBin {
            id,
            parent_id: Some(parent),
            name: unique_name,
            is_root: false,
            is_smart: false,
            search_criteria: Vec::new(),
            media_ids: HashSet::new(),
        };

        self.bins.insert(id, bin);
        Ok(id)
    }

    /// Creates a dynamic Smart Search Bin.
    pub fn create_smart_bin(
        &mut self,
        parent_id: Option<BinId>,
        requested_name: &str,
        criteria: Vec<SearchCriterion>,
    ) -> Result<BinId, AlertCode> {
        let bin_id = self.create_bin(parent_id, requested_name)?;
        if let Some(bin) = self.bins.get_mut(&bin_id) {
            bin.is_smart = true;
            bin.search_criteria = criteria;
        }
        Ok(bin_id)
    }

    /// Renames an existing bin.
    ///
    /// Rejects renaming root "All Media" (Alert ID 12889).
    /// Rejects empty name (Alert ID 12887).
    pub fn rename_bin(&mut self, id: BinId, new_name: &str) -> Result<(), AlertCode> {
        if id == self.root_id {
            return Err(AlertCode::RootBinCannotRename);
        }
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err(AlertCode::BinNameEmpty);
        }

        let parent_id = self
            .bins
            .get(&id)
            .and_then(|b| b.parent_id)
            .unwrap_or(self.root_id);
        let unique_name = self.resolve_unique_name(parent_id, trimmed, Some(id));

        if let Some(bin) = self.bins.get_mut(&id) {
            bin.name = unique_name;
            Ok(())
        } else {
            Err(AlertCode::ParametersInvalid)
        }
    }

    /// Deletes a bin by ID.
    ///
    /// Rejects deleting root "All Media" (Alert ID 12878).
    /// Media items inside deleted bins remain intact in All Media and timeline (Edge Case 20).
    pub fn delete_bin(&mut self, id: BinId) -> Result<(), AlertCode> {
        if id == self.root_id {
            return Err(AlertCode::RootBinCannotRemove);
        }

        // Recursively find and remove all children bins
        let children: Vec<BinId> = self
            .bins
            .values()
            .filter(|b| b.parent_id == Some(id))
            .map(|b| b.id)
            .collect();

        for child in children {
            let _ = self.delete_bin(child);
        }

        self.bins.remove(&id);
        Ok(())
    }

    /// Sets the label for a Quick Tag slot (1..=10, corresponding to Ctrl+1..Ctrl+0).
    pub fn set_quick_tag(&mut self, slot_1_to_10: usize, label: &str) {
        if (1..=10).contains(&slot_1_to_10) {
            self.quick_tags[slot_1_to_10 - 1] = label.trim().to_string();
        }
    }

    pub fn get_quick_tag(&self, slot_1_to_10: usize) -> Option<&str> {
        if (1..=10).contains(&slot_1_to_10) {
            Some(&self.quick_tags[slot_1_to_10 - 1])
        } else {
            None
        }
    }

    /// Applies a quick tag to a media ID, automatically creating or adding to a tag bin.
    pub fn apply_quick_tag(&mut self, slot_1_to_10: usize, media_id: u64) -> Option<String> {
        if !(1..=10).contains(&slot_1_to_10) {
            return None;
        }
        let tag = self.quick_tags[slot_1_to_10 - 1].clone();
        if tag.is_empty() {
            return None;
        }

        // Find or create tag bin under Tags root
        let tag_bin_id = if let Some(existing_id) = self.find_bin_by_name(self.tags_root_id, &tag) {
            existing_id
        } else {
            self.create_bin(Some(self.tags_root_id), &tag).ok()?
        };

        if let Some(bin) = self.bins.get_mut(&tag_bin_id) {
            bin.media_ids.insert(media_id);
        }

        Some(tag)
    }

    /// Evaluates all dynamic smart bins against current Project Media pool items.
    pub fn evaluate_smart_bins(&mut self, pool_items: &[MediaDetailRow]) {
        for bin in self.bins.values_mut() {
            if !bin.is_smart || bin.search_criteria.is_empty() {
                continue;
            }
            bin.media_ids.clear();
            for item in pool_items {
                // Must match ALL criteria (AND semantics)
                let matches_all = bin.search_criteria.iter().all(|crit| crit.matches(item));
                if matches_all {
                    bin.media_ids.insert(item.id);
                }
            }
        }
    }

    fn find_bin_by_name(&self, parent_id: BinId, name: &str) -> Option<BinId> {
        self.bins
            .values()
            .find(|b| b.parent_id == Some(parent_id) && b.name.eq_ignore_ascii_case(name))
            .map(|b| b.id)
    }

    fn resolve_unique_name(
        &self,
        parent_id: BinId,
        base_name: &str,
        except: Option<BinId>,
    ) -> String {
        let mut name = base_name.to_string();
        let mut count = 1;

        while self.bins.values().any(|b| {
            Some(b.id) != except
                && b.parent_id == Some(parent_id)
                && b.name.eq_ignore_ascii_case(&name)
        }) {
            name = format!("{base_name} ({count})");
            count += 1;
        }
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn root_bin_cannot_be_deleted_or_renamed() {
        let mut tree = MediaBinTree::new();
        assert_eq!(
            tree.delete_bin(MediaBinTree::ROOT_BIN_ID),
            Err(AlertCode::RootBinCannotRemove)
        );
        assert_eq!(
            tree.rename_bin(MediaBinTree::ROOT_BIN_ID, "New Name"),
            Err(AlertCode::RootBinCannotRename)
        );
    }

    #[test]
    fn duplicate_bin_name_auto_renamed() {
        let mut tree = MediaBinTree::new();
        let b1 = tree.create_bin(None, "B-Roll").unwrap();
        let b2 = tree.create_bin(None, "B-Roll").unwrap();

        assert_eq!(tree.get_bin(b1).unwrap().name, "B-Roll");
        assert_eq!(tree.get_bin(b2).unwrap().name, "B-Roll (1)");
    }

    #[test]
    fn renaming_a_bin_to_its_own_name_keeps_the_name() {
        let mut tree = MediaBinTree::new();
        let bin = tree.create_bin(None, "B-Roll").unwrap();
        tree.rename_bin(bin, "B-Roll").unwrap();
        assert_eq!(tree.get_bin(bin).unwrap().name, "B-Roll");
        // A real clash still gets a suffix
        let other = tree.create_bin(None, "Other").unwrap();
        tree.rename_bin(other, "B-Roll").unwrap();
        assert_eq!(tree.get_bin(other).unwrap().name, "B-Roll (1)");
    }

    #[test]
    fn quick_tags_and_auto_bin_creation() {
        let mut tree = MediaBinTree::new();
        tree.set_quick_tag(1, "Interviews");
        let tag = tree.apply_quick_tag(1, 101);
        assert_eq!(tag, Some("Interviews".into()));

        // Check that a bin named "Interviews" exists under Tags root
        let interview_bin = tree
            .all_bins()
            .find(|b| b.name == "Interviews")
            .expect("Interviews bin");
        assert!(interview_bin.media_ids.contains(&101));
    }

    #[test]
    fn smart_bin_evaluates_query_criteria() {
        let mut tree = MediaBinTree::new();
        let crit = vec![
            SearchCriterion::new(SearchField::Type, SearchCondition::Equals, "Audio"),
            SearchCriterion::new(SearchField::Name, SearchCondition::Contains, "Music"),
        ];
        let smart_id = tree.create_smart_bin(None, "Music Tracks", crit).unwrap();

        let mut row1 = MediaDetailRow::new(1, "Background Music 01", PathBuf::from("bgm.wav"));
        row1.media_type = "Audio".into();

        let mut row2 = MediaDetailRow::new(2, "Sound FX", PathBuf::from("sfx.wav"));
        row2.media_type = "Audio".into();

        let mut row3 = MediaDetailRow::new(3, "Music Video", PathBuf::from("mv.mp4"));
        row3.media_type = "Video".into();

        tree.evaluate_smart_bins(&[row1, row2, row3]);

        let smart_bin = tree.get_bin(smart_id).unwrap();
        assert!(smart_bin.media_ids.contains(&1));
        assert!(!smart_bin.media_ids.contains(&2));
        assert!(!smart_bin.media_ids.contains(&3));
    }
}
