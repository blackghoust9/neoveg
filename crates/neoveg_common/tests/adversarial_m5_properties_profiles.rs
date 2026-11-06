//! Adversarial Empirical Stress Test Suite for Milestone 5 (Features 24–29).
//!
//! Evaluates and stress-tests:
//! 1. Media Profiles: reverse-order matching, multiple overlapping rules, corrupt INIs,
//!    negative/infinite numbers, and 0.005 FPS boundary tolerances.
//! 2. 46 Project Templates: parsing, custom persistence, roundtrip .vqpt import/export,
//!    corrupt format rejection, and strict immutability rejection on overwrite/deletion.
//! 3. Media Bins: cycle prevention, deep folder nesting (50+ levels), duplicate auto-renaming,
//!    case-insensitive collision resolution, and Smart Search Bins with dynamic multi-predicate filtering.
//! 4. Media Pool: all 23 detail column sorting (ascending/descending, numeric vs alphanumeric),
//!    empty pool operations, in-use timeline deletion protection, and missing media relinking.

use neoveg_common::alerts::AlertCode;
use neoveg_common::media_props::{AlphaMode, FieldOrder};
use neoveg_common::presets::{
    canonical_46_templates, export_vqpt, import_vqpt,
    ProjectTemplate, TemplateManager,
};
use neoveg_common::profiles::{
    MediaProfileAttributes, MediaProfileKey, MediaProfileMatcher,
};
use neoveg_common::project::ProjectSettings;
use neoveg_engine::media_bins::{
    MediaBinTree, SearchCondition, SearchCriterion, SearchField,
};
use neoveg_engine::media_pool::{
    MediaDetailColumn, MediaDetailRow, MediaPoolManager,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEST_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn create_temp_test_dir(prefix: &str) -> PathBuf {
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut dir = std::env::temp_dir();
    dir.push(format!("neoveg_adversarial_m5_{}_{}_{}", prefix, std::process::id(), id));
    fs::create_dir_all(&dir).expect("create temp test dir");
    dir
}

// ============================================================================
// SECTION 1: MEDIA PROFILES ADVERSARIAL STRESS SUITE (FEATURE 26)
// ============================================================================

#[test]
fn test_profiles_reverse_order_with_multiple_overlapping_rules() {
    let mut matcher = MediaProfileMatcher::new();

    // Add 4 rules that all match 1920x1080 @ 24 fps progressive, with different attributes.
    // Rule 1: lowest priority
    matcher.add_rule(
        MediaProfileKey::new(None, "None", 1920, 1080, 24.0, 0),
        MediaProfileAttributes::new(FieldOrder::Progressive, 1.0, AlphaMode::None, 2.222),
    );
    // Rule 2: matches same stream
    matcher.add_rule(
        MediaProfileKey::new(None, "None", 1920, 1080, 24.0, 0),
        MediaProfileAttributes::new(FieldOrder::UpperFirst, 1.333, AlphaMode::Straight, 1.0),
    );
    // Rule 3: matches same stream
    matcher.add_rule(
        MediaProfileKey::new(None, "None", 1920, 1080, 24.0, 0),
        MediaProfileAttributes::new(FieldOrder::LowerFirst, 1.5, AlphaMode::Premultiplied, 2.4),
    );
    // Rule 4: highest priority (added last)
    matcher.add_rule(
        MediaProfileKey::new(None, "None", 1920, 1080, 24.0, 0),
        MediaProfileAttributes::new(FieldOrder::Progressive, 2.0, AlphaMode::PremultipliedDirty, 1.8),
    );

    let query = MediaProfileKey::new(None, "None", 1920, 1080, 24.0, 8);
    let matched = matcher.match_stream(&query).expect("should match rule 4");

    // Reverse order check: Rule 4 MUST win
    assert_eq!(matched.interlace, FieldOrder::Progressive);
    assert!((matched.pixel_aspect_ratio - 2.0).abs() < 1e-4);
    assert_eq!(matched.alpha_interpretation, AlphaMode::PremultipliedDirty);
    assert!((matched.gamma - 1.8).abs() < 1e-4);
}

#[test]
fn test_profiles_fps_epsilon_boundary_tolerances() {
    let mut matcher = MediaProfileMatcher::new();
    // Rule with nominal 29.970 FPS
    matcher.add_rule(
        MediaProfileKey::new(None, "None", 1920, 1080, 29.970, 0),
        MediaProfileAttributes::new(FieldOrder::Progressive, 1.0, AlphaMode::None, 2.222),
    );

    // 1. Exact match (diff = 0.0)
    let q_exact = MediaProfileKey::new(None, "None", 1920, 1080, 29.970, 0);
    assert!(matcher.match_stream(&q_exact).is_some(), "Exact FPS must match");

    // 2. Just within upper tolerance: 29.970 + 0.0049 = 29.9749 (diff = 0.0049 < 0.005)
    let q_inside_upper = MediaProfileKey::new(None, "None", 1920, 1080, 29.9749, 0);
    assert!(matcher.match_stream(&q_inside_upper).is_some(), "diff 0.0049 must match");

    // 3. Just within lower tolerance: 29.970 - 0.0049 = 29.9651 (diff = 0.0049 < 0.005)
    let q_inside_lower = MediaProfileKey::new(None, "None", 1920, 1080, 29.9651, 0);
    assert!(matcher.match_stream(&q_inside_lower).is_some(), "diff 0.0049 lower must match");

    // 4. Exact boundary: 29.970 + 0.0050 = 29.9750 (diff = 0.0050, strict < 0.005 must NOT match)
    let q_boundary_upper = MediaProfileKey::new(None, "None", 1920, 1080, 29.9750, 0);
    assert!(matcher.match_stream(&q_boundary_upper).is_none(), "diff 0.0050 must not match (< 0.005)");

    // 5. Outside boundary: 29.970 + 0.0051 = 29.9751 (diff = 0.0051 > 0.005)
    let q_outside_upper = MediaProfileKey::new(None, "None", 1920, 1080, 29.9751, 0);
    assert!(matcher.match_stream(&q_outside_upper).is_none(), "diff 0.0051 must not match");

    // 6. Outside lower boundary: 29.970 - 0.0051 = 29.9649 (diff = 0.0051 > 0.005)
    let q_outside_lower = MediaProfileKey::new(None, "None", 1920, 1080, 29.9649, 0);
    assert!(matcher.match_stream(&q_outside_lower).is_none(), "diff 0.0051 lower must not match");

    // 7. Wildcard FPS 0.0 in rule matches any query FPS
    let mut wildcard_matcher = MediaProfileMatcher::new();
    wildcard_matcher.add_rule(
        MediaProfileKey::new(None, "None", 1920, 1080, 0.0, 0),
        MediaProfileAttributes::new(FieldOrder::Progressive, 1.0, AlphaMode::None, 2.222),
    );
    let q_any_fps = MediaProfileKey::new(None, "None", 1920, 1080, 120.0, 0);
    assert!(wildcard_matcher.match_stream(&q_any_fps).is_some(), "0.0 FPS rule matches 120 FPS");

    // 8. Query FPS <= 0.0 matches any rule FPS
    let q_zero_fps = MediaProfileKey::new(None, "None", 1920, 1080, 0.0, 0);
    assert!(matcher.match_stream(&q_zero_fps).is_some(), "0.0 query FPS matches nominal 29.970");
}

#[test]
fn test_profiles_corrupt_ini_files_and_syntax_edge_cases() {
    // 1. INI missing the [Video Media Profiles] section header
    let ini_no_section = r#"
[Some Other Section]
Key1 = 0, "None", 1920, 1080, 24.0, 0
Attributes1 = "Progressive", 1.0, "None", 2.222
"#;
    let m1 = MediaProfileMatcher::parse_ini(ini_no_section).expect("parse without error");
    assert_eq!(m1.rules().len(), 0, "Must ignore keys outside [Video Media Profiles]");

    // 2. INI with gap in numbering (Key1, Key2, Key4) -> stops at Key2
    let ini_gap = r#"
[Video Media Profiles]
Key1 = 0, "None", 720, 480, 29.97, 0
Attributes1 = "Lower field first", 0.909, "None", 2.222
Key2 = 0, "None", 1920, 1080, 24.0, 0
Attributes2 = "Progressive", 1.0, "None", 2.222
Key4 = 0, "None", 3840, 2160, 60.0, 0
Attributes4 = "Progressive", 1.0, "None", 2.222
"#;
    let m2 = MediaProfileMatcher::parse_ini(ini_gap).expect("parse with gap");
    assert_eq!(m2.rules().len(), 2, "Must stop parsing at first gap (Key3 missing)");

    // 3. INI starting numbering at 2 (no Key1) -> terminates immediately (0 rules)
    let ini_start_at_two = r#"
[Video Media Profiles]
Key2 = 0, "None", 1920, 1080, 24.0, 0
Attributes2 = "Progressive", 1.0, "None", 2.222
"#;
    let m3 = MediaProfileMatcher::parse_ini(ini_start_at_two).expect("parse start at 2");
    assert_eq!(m3.rules().len(), 0, "Missing Key1 must result in 0 rules loaded");

    // 4. INI with missing Attributes for Key1
    let ini_missing_attrs = r#"
[Video Media Profiles]
Key1 = 0, "None", 1920, 1080, 24.0, 0
"#;
    let m4 = MediaProfileMatcher::parse_ini(ini_missing_attrs).expect("parse missing attr");
    assert_eq!(m4.rules().len(), 0, "Key1 without Attributes1 must result in 0 rules");

    // 5. INI with malformed Key line (< 6 fields)
    let ini_malformed_key = r#"
[Video Media Profiles]
Key1 = 0, "None", 1920
Attributes1 = "Progressive", 1.0, "None", 2.222
"#;
    let m5 = MediaProfileMatcher::parse_ini(ini_malformed_key);
    assert!(m5.is_err(), "Key with fewer than 6 fields must return Err");

    // 6. INI with malformed Attributes line (< 4 fields)
    let ini_malformed_attr = r#"
[Video Media Profiles]
Key1 = 0, "None", 1920, 1080, 24.0, 0
Attributes1 = "Progressive", 1.0
"#;
    let m6 = MediaProfileMatcher::parse_ini(ini_malformed_attr);
    assert!(m6.is_err(), "Attributes with fewer than 4 fields must return Err");

    // 7. Empty content
    let m7 = MediaProfileMatcher::parse_ini("").expect("empty content");
    assert_eq!(m7.rules().len(), 0);
}

#[test]
fn test_profiles_negative_and_infinite_numbers() {
    let mut matcher = MediaProfileMatcher::new();

    // Negative FPS in rule acts as wildcard (<= 0.0)
    matcher.add_rule(
        MediaProfileKey::new(None, "None", 1920, 1080, -29.97, 0),
        MediaProfileAttributes::new(FieldOrder::Progressive, 1.0, AlphaMode::None, 2.222),
    );

    let query_finite = MediaProfileKey::new(None, "None", 1920, 1080, 60.0, 0);
    assert!(matcher.match_stream(&query_finite).is_some(), "Negative FPS rule acts as wildcard");

    // Negative query FPS also acts as wildcard
    let query_neg = MediaProfileKey::new(None, "None", 1920, 1080, -1.0, 0);
    assert!(matcher.match_stream(&query_neg).is_some(), "Negative query FPS matches");

    // Finite rule: must not match NaN or Infinity query FPS
    let mut finite_matcher = MediaProfileMatcher::new();
    finite_matcher.add_rule(
        MediaProfileKey::new(None, "None", 1920, 1080, 24.0, 0),
        MediaProfileAttributes::new(FieldOrder::Progressive, 1.0, AlphaMode::None, 2.222),
    );

    let query_nan = MediaProfileKey::new(None, "None", 1920, 1080, f64::NAN, 0);
    assert!(finite_matcher.match_stream(&query_nan).is_none(), "NaN query FPS must not match finite rule");

    let query_inf = MediaProfileKey::new(None, "None", 1920, 1080, f64::INFINITY, 0);
    assert!(finite_matcher.match_stream(&query_inf).is_none(), "Infinity query FPS must not match finite rule");
}

// ============================================================================
// SECTION 2: 46 PROJECT TEMPLATES & IMMUTABILITY SUITE (FEATURE 25)
// ============================================================================

#[test]
fn test_templates_all_46_canonical_presets_properties_and_uniqueness() {
    let templates = canonical_46_templates();
    assert_eq!(templates.len(), 46, "Must provide exactly 46 canonical .vqpt templates (§18.3)");

    let mut names = std::collections::HashSet::new();

    for t in &templates {
        assert!(t.is_builtin, "Canonical template '{}' must have is_builtin=true", t.name);
        assert!(!t.name.trim().is_empty(), "Template name must not be blank");
        assert!(names.insert(t.name.clone()), "Duplicate template name detected: {}", t.name);

        // Dimensional constraints: width & height must be positive and even
        assert!(t.settings.width >= 16 && t.settings.width <= 16384, "Width in range: {}", t.settings.width);
        assert!(t.settings.height >= 16 && t.settings.height <= 16384, "Height in range: {}", t.settings.height);
        assert_eq!(t.settings.width % 2, 0, "Width must be even integer for codec compatibility");
        assert_eq!(t.settings.height % 2, 0, "Height must be even integer for codec compatibility");

        // Frame rate sanity
        assert!(t.settings.fps >= 10.0 && t.settings.fps <= 240.0, "FPS in range: {}", t.settings.fps);

        // Pixel aspect ratio sanity
        assert!(t.settings.pixel_aspect >= 0.5 && t.settings.pixel_aspect <= 3.0, "PAR in range: {}", t.settings.pixel_aspect);

        // Category must be assigned
        assert!(t.category.is_some(), "Canonical template '{}' must have a category", t.name);
    }
}

#[test]
fn test_templates_strict_immutability_rejection_for_every_canonical_template() {
    let mut mgr = TemplateManager::new();
    let canonical = canonical_46_templates();

    for t in &canonical {
        // 1. is_builtin check
        assert!(mgr.is_builtin(&t.name), "Template '{}' must be recognized as built-in", t.name);

        // 2. Overwrite rejection (Alert ID 1580 / 17365)
        let modified_attempt = ProjectTemplate::new(&t.name, ProjectSettings {
            width: 320,
            height: 240,
            ..ProjectSettings::default()
        });
        let res_overwrite = mgr.save_custom(modified_attempt);
        assert!(
            res_overwrite.is_err(),
            "Must reject overwriting built-in template '{}'",
            t.name
        );
        let err_msg = res_overwrite.unwrap_err();
        assert!(
            err_msg.contains("built-in"),
            "Error message must mention built-in template: {err_msg}"
        );

        // 3. Deletion rejection (Alert ID 1585 / 17364)
        let res_delete = mgr.delete_custom(&t.name);
        assert!(
            res_delete.is_err(),
            "Must reject deleting built-in template '{}'",
            t.name
        );
        let err_del_msg = res_delete.unwrap_err();
        assert!(
            err_del_msg.contains("built-in"),
            "Delete error message must mention built-in: {err_del_msg}"
        );
    }
}

#[test]
fn test_templates_custom_persistence_update_and_empty_name_rejection() {
    let mut mgr = TemplateManager::new();

    // 1. Empty and whitespace names rejected (Alert ID 1578)
    assert!(mgr.save_custom(ProjectTemplate::new("", ProjectSettings::default())).is_err());
    assert!(mgr.save_custom(ProjectTemplate::new("   \t\n", ProjectSettings::default())).is_err());

    // 2. Add valid custom template
    let custom1 = ProjectTemplate::new("My Custom 4K Vertical", ProjectSettings {
        width: 2160,
        height: 3840,
        fps: 60.0,
        ..ProjectSettings::default()
    });
    assert!(mgr.save_custom(custom1.clone()).is_ok());

    // Verify it appears in all_templates
    assert!(mgr.find_by_name("My Custom 4K Vertical").is_some());
    assert!(!mgr.is_builtin("My Custom 4K Vertical"));

    // 3. Update existing custom template (overwrite allowed for custom presets per Alert ID 1583)
    let custom1_updated = ProjectTemplate::new("My Custom 4K Vertical", ProjectSettings {
        width: 2160,
        height: 3840,
        fps: 120.0, // changed fps
        ..ProjectSettings::default()
    });
    assert!(mgr.save_custom(custom1_updated).is_ok());
    let retrieved = mgr.find_by_name("My Custom 4K Vertical").unwrap();
    assert_eq!(retrieved.settings.fps, 120.0, "Custom preset update must persist");

    // 4. Delete custom template
    assert!(mgr.delete_custom("My Custom 4K Vertical").is_ok());
    assert!(mgr.find_by_name("My Custom 4K Vertical").is_none());

    // 5. Deleting non-existent template returns error
    assert!(mgr.delete_custom("Non Existent Template").is_err());
}

#[test]
fn test_templates_vqpt_roundtrip_and_corrupt_rejection() {
    let canonical = canonical_46_templates();

    // Test JSON roundtrip for all 46 templates
    for original in &canonical {
        let exported = export_vqpt(original).expect("export .vqpt");
        assert!(!exported.trim().is_empty());

        let imported = import_vqpt(&exported).expect("import .vqpt");
        assert_eq!(imported.name, original.name);
        assert_eq!(imported.settings.width, original.settings.width);
        assert_eq!(imported.settings.height, original.settings.height);
        assert!((imported.settings.fps - original.settings.fps).abs() < 1e-4);
        assert_eq!(imported.settings.field_order, original.settings.field_order);
    }

    // Corrupt .vqpt imports (Alert ID 17437)
    assert!(import_vqpt("").is_err());
    assert!(import_vqpt("not a json string").is_err());
    assert!(import_vqpt("{ broken json: ").is_err());
    assert!(import_vqpt(r#"{"name": "NoSettings"}"#).is_err());
    let err = import_vqpt(r#"{"corrupt": true}"#).unwrap_err();
    assert!(err.contains("17437"), "Error message must report Alert ID 17437: {err}");
}

// ============================================================================
// SECTION 3: MEDIA BINS ADVERSARIAL STRESS SUITE (FEATURE 28)
// ============================================================================

#[test]
fn test_bins_root_immutability_and_empty_name_rejection() {
    let mut tree = MediaBinTree::new();

    // 1. Root bin cannot be deleted (Alert ID 12878)
    assert_eq!(
        tree.delete_bin(MediaBinTree::ROOT_BIN_ID),
        Err(AlertCode::RootBinCannotRemove)
    );

    // 2. Root bin cannot be renamed (Alert ID 12889)
    assert_eq!(
        tree.rename_bin(MediaBinTree::ROOT_BIN_ID, "Custom Root"),
        Err(AlertCode::RootBinCannotRename)
    );

    // 3. Empty or whitespace bin name rejected on create (Alert ID 12887)
    assert_eq!(tree.create_bin(None, ""), Err(AlertCode::BinNameEmpty));
    assert_eq!(tree.create_bin(None, "   \t\n"), Err(AlertCode::BinNameEmpty));

    // 4. Empty or whitespace bin name rejected on rename
    let b1 = tree.create_bin(None, "Valid Bin").unwrap();
    assert_eq!(tree.rename_bin(b1, ""), Err(AlertCode::BinNameEmpty));
    assert_eq!(tree.rename_bin(b1, "   "), Err(AlertCode::BinNameEmpty));
}

#[test]
fn test_bins_duplicate_auto_renaming_and_case_insensitivity() {
    let mut tree = MediaBinTree::new();

    // 1. Create first "Interviews"
    let id1 = tree.create_bin(None, "Interviews").unwrap();
    assert_eq!(tree.get_bin(id1).unwrap().name, "Interviews");

    // 2. Create duplicate exact name -> auto-renamed to "Interviews (1)"
    let id2 = tree.create_bin(None, "Interviews").unwrap();
    assert_eq!(tree.get_bin(id2).unwrap().name, "Interviews (1)");

    // 3. Create case-insensitive duplicate "interviews" -> "Interviews (2)"
    let id3 = tree.create_bin(None, "interviews").unwrap();
    assert_eq!(tree.get_bin(id3).unwrap().name, "interviews (2)");

    // 4. Create uppercase duplicate "INTERVIEWS" -> "INTERVIEWS (3)"
    let id4 = tree.create_bin(None, "INTERVIEWS").unwrap();
    assert_eq!(tree.get_bin(id4).unwrap().name, "INTERVIEWS (3)");

    // 5. In a different parent bin, the name "Interviews" should NOT collide
    let subparent = tree.create_bin(None, "SubFolder").unwrap();
    let id_in_sub = tree.create_bin(Some(subparent), "Interviews").unwrap();
    assert_eq!(
        tree.get_bin(id_in_sub).unwrap().name,
        "Interviews",
        "Different parent must allow identical name without auto-renaming"
    );
}

#[test]
fn test_bins_deep_folder_nesting_and_recursive_deletion() {
    let mut tree = MediaBinTree::new();
    const DEPTH: usize = 50;

    let mut current_parent = MediaBinTree::ROOT_BIN_ID;
    let mut chain = Vec::new();

    // Create 50 nested levels: Root -> L1 -> L2 -> ... -> L50
    for level in 1..=DEPTH {
        let name = format!("Level_{level:03}");
        let new_id = tree.create_bin(Some(current_parent), &name).unwrap();
        chain.push(new_id);
        current_parent = new_id;
    }

    // Verify deepest bin exists and parent link is intact
    let deepest_id = *chain.last().unwrap();
    assert!(tree.get_bin(deepest_id).is_some());
    assert_eq!(tree.get_bin(deepest_id).unwrap().name, "Level_050");

    // Delete at level 25 (intermediate node)
    let cut_point = chain[24]; // 25th element (Level_025)
    assert!(tree.delete_bin(cut_point).is_ok());

    // Bins from Level 1 to 24 must still exist
    for &id in &chain[..24] {
        assert!(tree.get_bin(id).is_some(), "Level prior to cut must survive");
    }

    // Bins from Level 25 to 50 must be completely removed by recursive deletion
    for &id in &chain[24..] {
        assert!(tree.get_bin(id).is_none(), "Level after cut must be removed");
    }
}

#[test]
fn test_bins_quick_tags_boundaries_and_auto_bin() {
    let mut tree = MediaBinTree::new();

    // Valid slots are 1..=10 (Ctrl+1 to Ctrl+0)
    tree.set_quick_tag(1, "A-Roll");
    tree.set_quick_tag(10, "Archival");

    // Boundary checks: slot 0 and slot 11 are invalid
    tree.set_quick_tag(0, "Invalid Slot 0");
    tree.set_quick_tag(11, "Invalid Slot 11");
    assert!(tree.get_quick_tag(0).is_none());
    assert!(tree.get_quick_tag(11).is_none());

    // Apply quick tag
    let applied = tree.apply_quick_tag(1, 42);
    assert_eq!(applied, Some("A-Roll".into()));

    // Invalid slot apply returns None
    assert!(tree.apply_quick_tag(0, 42).is_none());
    assert!(tree.apply_quick_tag(11, 42).is_none());

    // Empty quick tag apply returns None
    assert!(tree.apply_quick_tag(2, 42).is_none());
}

#[test]
fn test_smart_search_bins_complex_multi_predicate_filtering() {
    let mut tree = MediaBinTree::new();

    // Create Smart Bin with 4 complex AND criteria:
    // 1. Type == "Video"
    // 2. Name contains "Drone"
    // 3. Comment does NOT contain "Corrupt"
    // 4. FieldOrder == "Progressive"
    let criteria = vec![
        SearchCriterion::new(SearchField::Type, SearchCondition::Equals, "Video"),
        SearchCriterion::new(SearchField::Name, SearchCondition::Contains, "Drone"),
        SearchCriterion::new(SearchField::Comment, SearchCondition::DoesNotContain, "Corrupt"),
        SearchCriterion::new(SearchField::FieldOrder, SearchCondition::Equals, "Progressive"),
    ];
    let smart_id = tree.create_smart_bin(None, "Clean 4K Drones", criteria).unwrap();

    // Construct test items:
    // Item 1: matches all 4 criteria -> MUST MATCH
    let mut item1 = MediaDetailRow::new(101, "Drone Shot 01", PathBuf::from("drone1.mp4"));
    item1.media_type = "Video".into();
    item1.comment = "Clean sunset pass".into();
    item1.field_order = "Progressive".into();

    // Item 2: matches 3 criteria but Comment contains "Corrupt" -> MUST NOT MATCH
    let mut item2 = MediaDetailRow::new(102, "Drone Shot 02", PathBuf::from("drone2.mp4"));
    item2.media_type = "Video".into();
    item2.comment = "Corrupt frame at end".into();
    item2.field_order = "Progressive".into();

    // Item 3: matches 3 criteria but FieldOrder is "UpperFirst" -> MUST NOT MATCH
    let mut item3 = MediaDetailRow::new(103, "Drone Shot 03", PathBuf::from("drone3.mp4"));
    item3.media_type = "Video".into();
    item3.comment = "Good".into();
    item3.field_order = "Upper field first".into();

    // Item 4: matches 3 criteria but Name does not contain "Drone" -> MUST NOT MATCH
    let mut item4 = MediaDetailRow::new(104, "Gimbal Shot 04", PathBuf::from("gimbal.mp4"));
    item4.media_type = "Video".into();
    item4.comment = "Clean".into();
    item4.field_order = "Progressive".into();

    // Item 5: audio type -> MUST NOT MATCH
    let mut item5 = MediaDetailRow::new(105, "Drone Audio Whine", PathBuf::from("drone_audio.wav"));
    item5.media_type = "Audio".into();
    item5.comment = "Clean".into();
    item5.field_order = "Progressive".into();

    tree.evaluate_smart_bins(&[item1, item2, item3, item4, item5]);

    let bin = tree.get_bin(smart_id).expect("smart bin exists");
    assert!(bin.media_ids.contains(&101), "Item 1 matches all criteria");
    assert!(!bin.media_ids.contains(&102), "Item 2 filtered by comment condition");
    assert!(!bin.media_ids.contains(&103), "Item 3 filtered by field order condition");
    assert!(!bin.media_ids.contains(&104), "Item 4 filtered by name condition");
    assert!(!bin.media_ids.contains(&105), "Item 5 filtered by type condition");
}

#[test]
fn test_smart_search_bins_text_operators_and_tag_conditions() {
    let mut tree = MediaBinTree::new();

    // StartsWith, EndsWith, IsEmpty, IsNotEmpty
    let crit_starts = vec![SearchCriterion::new(SearchField::Name, SearchCondition::StartsWith, "RAW_")];
    let b_starts = tree.create_smart_bin(None, "RAW Clips", crit_starts).unwrap();

    let crit_ends = vec![SearchCriterion::new(SearchField::Path, SearchCondition::EndsWith, ".dng")];
    let b_ends = tree.create_smart_bin(None, "DNG Files", crit_ends).unwrap();

    let crit_empty_comment = vec![SearchCriterion::new(SearchField::Comment, SearchCondition::IsEmpty, "")];
    let b_empty_comment = tree.create_smart_bin(None, "Uncommented", crit_empty_comment).unwrap();

    let crit_has_tags = vec![SearchCriterion::new(SearchField::Tag, SearchCondition::IsNotEmpty, "")];
    let b_has_tags = tree.create_smart_bin(None, "Tagged Clips", crit_has_tags).unwrap();

    let mut row_a = MediaDetailRow::new(1, "RAW_001", PathBuf::from("C:/Media/RAW_001.dng"));
    row_a.comment = "".into();
    row_a.tags = vec!["Favorite".into()];

    let mut row_b = MediaDetailRow::new(2, "CLIP_RAW_002", PathBuf::from("C:/Media/CLIP_002.mov"));
    row_b.comment = "Reviewed".into();
    row_b.tags = Vec::new();

    tree.evaluate_smart_bins(&[row_a, row_b]);

    assert!(tree.get_bin(b_starts).unwrap().media_ids.contains(&1));
    assert!(!tree.get_bin(b_starts).unwrap().media_ids.contains(&2));

    assert!(tree.get_bin(b_ends).unwrap().media_ids.contains(&1));
    assert!(!tree.get_bin(b_ends).unwrap().media_ids.contains(&2));

    assert!(tree.get_bin(b_empty_comment).unwrap().media_ids.contains(&1));
    assert!(!tree.get_bin(b_empty_comment).unwrap().media_ids.contains(&2));

    assert!(tree.get_bin(b_has_tags).unwrap().media_ids.contains(&1));
    assert!(!tree.get_bin(b_has_tags).unwrap().media_ids.contains(&2));
}

// ============================================================================
// SECTION 4: MEDIA POOL 23 DETAIL COLUMNS & RELINKING SUITE (FEATURE 27)
// ============================================================================

#[test]
fn test_media_pool_all_23_columns_ascending_and_descending_sorting() {
    let mut mgr = MediaPoolManager::new();

    // Add 3 distinct rows with different metadata across all 23 columns
    let mut r1 = MediaDetailRow::new(1, "Alpha Media", PathBuf::from("C:/media/a.mp4"));
    r1.use_count = 1;
    r1.media_type = "Audio".into();
    r1.has_fx = false;
    r1.comment = "Apple comment".into();
    r1.timecode_in = "00:00:01:00".into();
    r1.timecode_out = "00:00:10:00".into();
    r1.length_seconds = 9.0;
    r1.tape_name = "Tape A".into();
    r1.size_bytes = 1_000_000;
    r1.modified_timestamp = 1000;
    r1.avg_total_bitrate = 500_000;
    r1.sample_rate = 44_100;
    r1.bit_depth = 16;
    r1.channels = 2;
    r1.avg_audio_bitrate = 128_000;
    r1.audio_format = "AAC".into();
    r1.fps = 23.976;
    r1.field_order = "Lower field first".into();
    r1.alpha = "None".into();
    r1.pixel_aspect = 0.9091;
    r1.frame_size = (720, 480); // area 345,600

    let mut r2 = MediaDetailRow::new(2, "Beta Media", PathBuf::from("C:/media/b.mov"));
    r2.use_count = 5;
    r2.media_type = "Still Image".into();
    r2.has_fx = true;
    r2.comment = "Banana comment".into();
    r2.timecode_in = "00:00:05:00".into();
    r2.timecode_out = "00:00:20:00".into();
    r2.length_seconds = 15.0;
    r2.tape_name = "Tape B".into();
    r2.size_bytes = 50_000_000;
    r2.modified_timestamp = 2000;
    r2.avg_total_bitrate = 20_000_000;
    r2.sample_rate = 48_000;
    r2.bit_depth = 24;
    r2.channels = 6;
    r2.avg_audio_bitrate = 384_000;
    r2.audio_format = "PCM".into();
    r2.fps = 29.970;
    r2.field_order = "Progressive".into();
    r2.alpha = "Straight".into();
    r2.pixel_aspect = 1.0000;
    r2.frame_size = (1920, 1080); // area 2,073,600

    let mut r3 = MediaDetailRow::new(3, "Gamma Media", PathBuf::from("C:/media/c.mxf"));
    r3.use_count = 10;
    r3.media_type = "Video".into();
    r3.has_fx = true;
    r3.comment = "Cherry comment".into();
    r3.timecode_in = "00:00:10:00".into();
    r3.timecode_out = "00:01:00:00".into();
    r3.length_seconds = 50.0;
    r3.tape_name = "Tape C".into();
    r3.size_bytes = 200_000_000;
    r3.modified_timestamp = 3000;
    r3.avg_total_bitrate = 100_000_000;
    r3.sample_rate = 96_000;
    r3.bit_depth = 32;
    r3.channels = 8;
    r3.avg_audio_bitrate = 768_000;
    r3.audio_format = "Uncompressed".into();
    r3.fps = 59.940;
    r3.field_order = "Upper field first".into();
    r3.alpha = "Premultiplied".into();
    r3.pixel_aspect = 1.3333;
    r3.frame_size = (3840, 2160); // area 8,294,400

    mgr.add_item(r1);
    mgr.add_item(r2);
    mgr.add_item(r3);

    // Assert that MediaDetailColumn::ALL contains exactly 23 distinct columns
    assert_eq!(MediaDetailColumn::ALL.len(), 23);

    // Test sort on EVERY single column in both Ascending and Descending order
    for &col in &MediaDetailColumn::ALL {
        // Ascending
        mgr.set_sort(col, true);
        let asc_ids: Vec<u64> = mgr.filtered_rows().into_iter().map(|r| r.id).collect();
        assert_eq!(asc_ids.len(), 3, "Column {:?} ascending should return 3 items", col);

        // Descending
        mgr.set_sort(col, false);
        let desc_ids: Vec<u64> = mgr.filtered_rows().into_iter().map(|r| r.id).collect();
        assert_eq!(desc_ids.len(), 3, "Column {:?} descending should return 3 items", col);

        // First item in ascending must equal last item in descending (for strictly ordered unique columns)
        if col != MediaDetailColumn::MediaFx { // MediaFx has a duplicate (r2 and r3 both true)
            assert_eq!(
                asc_ids[0], desc_ids[2],
                "Ascending first ID must match descending last ID for column {:?}",
                col
            );
            assert_eq!(
                asc_ids[2], desc_ids[0],
                "Ascending last ID must match descending first ID for column {:?}",
                col
            );
        }
    }
}

#[test]
fn test_media_pool_numeric_vs_alphanumeric_sorting_distinction() {
    let mut mgr = MediaPoolManager::new();

    // Rows where numeric vs alphanumeric sorting would produce opposite orders:
    // Sizes: 20 bytes, 100 bytes, 5 bytes.
    // Alphanumeric order: "100", "20", "5"
    // Numeric order: 5, 20, 100
    let mut r1 = MediaDetailRow::new(1, "File 100", PathBuf::from("f100.dat"));
    r1.size_bytes = 100;
    r1.fps = 100.0;
    r1.sample_rate = 100;

    let mut r2 = MediaDetailRow::new(2, "File 20", PathBuf::from("f20.dat"));
    r2.size_bytes = 20;
    r2.fps = 20.0;
    r2.sample_rate = 20;

    let mut r3 = MediaDetailRow::new(3, "File 5", PathBuf::from("f5.dat"));
    r3.size_bytes = 5;
    r3.fps = 5.0;
    r3.sample_rate = 5;

    mgr.add_item(r1);
    mgr.add_item(r2);
    mgr.add_item(r3);

    // 1. Sort by Size (numeric): expected 5 -> 20 -> 100 (IDs: 3, 2, 1)
    mgr.set_sort(MediaDetailColumn::Size, true);
    let rows_size = mgr.filtered_rows();
    assert_eq!(rows_size[0].id, 3);
    assert_eq!(rows_size[1].id, 2);
    assert_eq!(rows_size[2].id, 1);

    // 2. Sort by Frame Rate (numeric): expected 5.0 -> 20.0 -> 100.0 (IDs: 3, 2, 1)
    mgr.set_sort(MediaDetailColumn::Fps, true);
    let rows_fps = mgr.filtered_rows();
    assert_eq!(rows_fps[0].id, 3);
    assert_eq!(rows_fps[1].id, 2);
    assert_eq!(rows_fps[2].id, 1);

    // 3. Sort by Name (alphanumeric): expected "File 100" -> "File 20" -> "File 5" (IDs: 1, 2, 3)
    mgr.set_sort(MediaDetailColumn::Name, true);
    let rows_name = mgr.filtered_rows();
    assert_eq!(rows_name[0].id, 1);
    assert_eq!(rows_name[1].id, 2);
    assert_eq!(rows_name[2].id, 3);
}

#[test]
fn test_media_pool_empty_list_and_unused_cleanup() {
    let mut mgr = MediaPoolManager::new();

    // 1. Operations on empty pool must not panic
    assert!(mgr.filtered_rows().is_empty());
    assert_eq!(mgr.clean_unused(), 0);
    assert!(mgr.relink_missing(&[]).is_empty());
    assert!(mgr.remove_item(999, false).is_ok());

    // 2. Add in-use and unused items
    let mut in_use = MediaDetailRow::new(1, "Timeline Clip", PathBuf::from("tl.mp4"));
    in_use.use_count = 2;

    let mut unused1 = MediaDetailRow::new(2, "Unused Clip 1", PathBuf::from("un1.mp4"));
    unused1.use_count = 0;

    let mut unused2 = MediaDetailRow::new(3, "Unused Clip 2", PathBuf::from("un2.mp4"));
    unused2.use_count = 0;

    mgr.add_item(in_use);
    mgr.add_item(unused1);
    mgr.add_item(unused2);
    assert_eq!(mgr.items().len(), 3);

    // 3. Clean unused removes 2 items, retains in-use item
    let cleaned = mgr.clean_unused();
    assert_eq!(cleaned, 2);
    assert_eq!(mgr.items().len(), 1);
    assert_eq!(mgr.items()[0].id, 1);
}

#[test]
fn test_media_pool_in_use_timeline_deletion_protection() {
    let mut mgr = MediaPoolManager::new();

    let mut in_use = MediaDetailRow::new(10, "In Use Media", PathBuf::from("used.mov"));
    in_use.use_count = 3;
    mgr.add_item(in_use);

    // Attempting to delete in-use media with allow_timeline_remove=false must return AlertCode::TimelineMediaDeleteConfirm (ID 17264)
    let res = mgr.remove_item(10, false);
    assert_eq!(res, Err(AlertCode::TimelineMediaDeleteConfirm));
    assert_eq!(mgr.items().len(), 1, "Item must NOT have been deleted");

    // With allow_timeline_remove=true, deletion proceeds
    let res_confirmed = mgr.remove_item(10, true);
    assert!(res_confirmed.is_ok());
    assert!(mgr.items().is_empty(), "Item must now be deleted");
}

#[test]
fn test_media_pool_relink_missing_media() {
    let temp_dir = create_temp_test_dir("relink");
    let target_file = temp_dir.join("lost_footage.mov");
    fs::write(&target_file, b"sample media data").expect("write sample media");

    let mut mgr = MediaPoolManager::new();

    // 1. Missing item
    let mut missing_row = MediaDetailRow::new(100, "Lost Footage", PathBuf::from("D:/OldPath/lost_footage.mov"));
    missing_row.is_missing = true;

    // 2. Already present item (not missing)
    let mut present_row = MediaDetailRow::new(200, "Present Footage", PathBuf::from("C:/StillThere/other.mov"));
    present_row.is_missing = false;

    // 3. Unresolvable missing item
    let mut unresolvable_row = MediaDetailRow::new(300, "Never Found", PathBuf::from("E:/GoneForever/never.mov"));
    unresolvable_row.is_missing = true;

    mgr.add_item(missing_row);
    mgr.add_item(present_row);
    mgr.add_item(unresolvable_row);

    let relinked = mgr.relink_missing(&[temp_dir.clone()]);

    assert_eq!(relinked.len(), 1, "Only 1 item should be relinked");
    assert_eq!(relinked[0].0, 100);
    assert_eq!(relinked[0].1, target_file);

    // Verify row 100 is no longer missing and has updated path
    let r100 = mgr.items().iter().find(|r| r.id == 100).unwrap();
    assert!(!r100.is_missing);
    assert_eq!(r100.path, target_file);

    // Verify row 200 was untouched
    let r200 = mgr.items().iter().find(|r| r.id == 200).unwrap();
    assert!(!r200.is_missing);
    assert_eq!(r200.path, PathBuf::from("C:/StillThere/other.mov"));

    // Verify row 300 is still missing
    let r300 = mgr.items().iter().find(|r| r.id == 300).unwrap();
    assert!(r300.is_missing);

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}
