//! Arquivo > Importar > *Legendas das regiões*: each named region becomes a subtitle (the rule is
//! `neoveg_engine::caption_table::from_regions`); one undo step.

use neoveg_common::i18n::tr_args;
use neoveg_engine::caption_table;

use super::*;

impl AppState {
    pub(super) fn captions_from_regions(&mut self) {
        let doc = caption_table::from_regions(&self.timeline);
        let count = doc.cues.len();
        match captions::import_document(&mut self.timeline, &doc, 0.0, false) {
            Some(command) => {
                self.execute_timeline(command);
                self.status = tr_args("{} legendas criadas das regiões", &[&count]);
            }
            None => self.status = "Nenhuma região com nome para virar legenda".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_regions_become_captions_and_undo_removes_them() {
        let mut s = AppState::new();
        s.timeline.regions.push(neoveg_engine::markers::Region {
            id: 1,
            start: 0.0,
            end: 2.0,
            name: "Abertura".into(),
        });
        s.captions_from_regions();
        assert_eq!(s.timeline.captions.len(), 1);
        assert_eq!(s.timeline.captions[0].text, "Abertura");
        s.history.undo(&mut s.timeline);
        assert!(s.timeline.captions.is_empty());
    }
}
