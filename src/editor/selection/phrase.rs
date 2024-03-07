use std::cmp::min;

use crate::{
    conversion::{Break, Composition, ConversionEngine, Interval, Symbol},
    dictionary::{Dictionary, LayeredDictionary},
    editor::Editor,
};

#[derive(Debug)]
pub(crate) struct PhraseSelector {
    begin: usize,
    end: usize,
    forward_select: bool,
    orig: usize,
    buffer: Vec<Symbol>,
    selections: Vec<Interval>,
    breaks: Vec<Break>,
}

impl PhraseSelector {
    pub(crate) fn new(forward_select: bool, com: Composition) -> PhraseSelector {
        PhraseSelector {
            begin: 0,
            end: com.buffer.len(),
            forward_select,
            orig: 0,
            buffer: com.buffer,
            selections: com.selections,
            breaks: com.breaks,
        }
    }

    pub(crate) fn init<D: Dictionary>(&mut self, cursor: usize, dict: &D) {
        self.orig = cursor;
        if self.forward_select {
            self.begin = if cursor == self.buffer.len() {
                cursor - 1
            } else {
                cursor
            };
            self.end = self.next_break_point(cursor);
        } else {
            self.end = min(cursor + 1, self.buffer.len());
            self.begin = self.after_previous_break_point(cursor);
        }
        loop {
            let syllables = &self.buffer[self.begin..self.end];
            debug_assert!(
                !syllables.is_empty(),
                "should not enter here if there's no syllable in range"
            );
            if dict.lookup_first_phrase(&syllables).is_some() {
                break;
            }
            if self.forward_select {
                self.end -= 1;
            } else {
                self.begin += 1;
            }
        }
    }

    pub(crate) fn begin(&self) -> usize {
        self.begin
    }

    pub(crate) fn next_selection_point<D: Dictionary>(&self, dict: &D) -> Option<(usize, usize)> {
        let (mut begin, mut end) = (self.begin, self.end);
        loop {
            if self.forward_select {
                end -= 1;
                if begin == end {
                    return None;
                }
            } else {
                begin += 1;
                if begin == end {
                    return None;
                }
            }
            let syllables = &self.buffer[begin..end];
            if dict.lookup_first_phrase(&syllables).is_some() {
                return Some((begin, end));
            }
        }
    }
    pub(crate) fn prev_selection_point<D: Dictionary>(&self, dict: &D) -> Option<(usize, usize)> {
        let (mut begin, mut end) = (self.begin, self.end);
        loop {
            if self.forward_select {
                if end == self.buffer.len() {
                    return None;
                }
                end += 1;
                if end > self.next_break_point(self.orig) {
                    return None;
                }
            } else {
                if begin == 0 {
                    return None;
                }
                begin -= 1;
                if begin < self.after_previous_break_point(self.orig) {
                    return None;
                }
            }
            let syllables = &self.buffer[begin..end];
            if dict.lookup_first_phrase(&syllables).is_some() {
                return Some((begin, end));
            }
        }
    }
    pub(crate) fn jump_to_next_selection_point<D: Dictionary>(
        &mut self,
        dict: &D,
    ) -> Result<(), ()> {
        if let Some((begin, end)) = self.next_selection_point(dict) {
            self.begin = begin;
            self.end = end;
            Ok(())
        } else {
            Err(())
        }
    }
    pub(crate) fn jump_to_prev_selection_point<D: Dictionary>(
        &mut self,
        dict: &D,
    ) -> Result<(), ()> {
        if let Some((begin, end)) = self.prev_selection_point(dict) {
            self.begin = begin;
            self.end = end;
            Ok(())
        } else {
            Err(())
        }
    }
    pub(crate) fn jump_to_first_selection_point<D: Dictionary>(&mut self, dict: &D) {
        self.init(self.orig, dict);
    }
    pub(crate) fn jump_to_last_selection_point<D: Dictionary>(&mut self, dict: &D) {
        while let Some(_) = self.next_selection_point(dict) {
            let _ = self.jump_to_next_selection_point(dict);
        }
    }

    pub(crate) fn next<D: Dictionary>(&mut self, dict: &D) {
        loop {
            if self.forward_select {
                self.end -= 1;
                if self.begin == self.end {
                    self.end = self.next_break_point(self.begin);
                }
            } else {
                self.begin += 1;
                if self.begin == self.end {
                    self.begin = self.after_previous_break_point(self.begin);
                }
            }
            let syllables = &self.buffer[self.begin..self.end];
            if dict.lookup_first_phrase(&syllables).is_some() {
                break;
            }
        }
    }

    fn next_break_point(&self, mut cursor: usize) -> usize {
        loop {
            if self.buffer.len() == cursor {
                break;
            }
            if !self.buffer[cursor].is_syllable() {
                break;
            }
            cursor += 1;
        }
        cursor
    }

    fn after_previous_break_point(&self, mut cursor: usize) -> usize {
        let selection_ends: Vec<_> = self.selections.iter().map(|sel| sel.end).collect();
        loop {
            if cursor == 0 {
                return 0;
            }
            if selection_ends.binary_search(&cursor).is_ok() {
                break;
            }
            if self.breaks.binary_search(&Break(cursor)).is_ok() {
                break;
            }
            if !self.buffer[cursor - 1].is_syllable() {
                break;
            }
            cursor -= 1;
        }
        cursor
    }

    pub(crate) fn candidates<C: ConversionEngine<LayeredDictionary>>(
        &self,
        editor: &Editor<C>,
        dict: &LayeredDictionary,
    ) -> Vec<String> {
        let mut candidates = dict
            .lookup_all_phrases(&&self.buffer[self.begin..self.end])
            .into_iter()
            .map(|phrase| phrase.into())
            .collect::<Vec<String>>();
        if self.end - self.begin == 1 {
            let alt = editor
                .syl
                .alt_syllables(self.buffer[self.begin].as_syllable());
            for &syl in alt {
                candidates.extend(
                    dict.lookup_all_phrases(&[syl])
                        .into_iter()
                        .map(|ph| ph.into()),
                )
            }
        }
        candidates
    }

    pub(crate) fn interval(&self, phrase: String) -> Interval {
        Interval {
            start: self.begin,
            end: self.end,
            is_phrase: true,
            phrase,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        conversion::{Interval, Symbol},
        syl,
        zhuyin::Bopomofo::*,
    };

    use super::PhraseSelector;

    #[test]
    fn init_when_cursor_end_of_buffer_syllable() {
        let mut sel = PhraseSelector {
            begin: 0,
            end: 1,
            forward_select: false,
            orig: 0,
            buffer: vec![Symbol::Syllable(syl![C, E, TONE4])],
            selections: vec![],
            breaks: vec![],
        };
        let dict = HashMap::from([(vec![syl![C, E, TONE4]], vec![("測", 100).into()])]);
        sel.init(1, &dict);

        assert_eq!(0, sel.begin);
        assert_eq!(1, sel.end);
    }

    #[test]
    #[should_panic]
    fn init_when_cursor_end_of_buffer_not_syllable() {
        let mut sel = PhraseSelector {
            begin: 0,
            end: 1,
            forward_select: false,
            orig: 0,
            buffer: vec![Symbol::Char(',')],
            selections: vec![],
            breaks: vec![],
        };
        let dict = HashMap::from([(vec![syl![C, E, TONE4]], vec![("測", 100).into()])]);
        sel.init(1, &dict);
    }

    #[test]
    fn init_forward_select_when_cursor_end_of_buffer_syllable() {
        let mut sel = PhraseSelector {
            begin: 0,
            end: 1,
            forward_select: true,
            orig: 0,
            buffer: vec![Symbol::Syllable(syl![C, E, TONE4])],
            selections: vec![],
            breaks: vec![],
        };
        let dict = HashMap::from([(vec![syl![C, E, TONE4]], vec![("測", 100).into()])]);
        sel.init(1, &dict);

        assert_eq!(0, sel.begin);
        assert_eq!(1, sel.end);
    }

    #[test]
    #[should_panic]
    fn init_forward_select_when_cursor_end_of_buffer_not_syllable() {
        let mut sel = PhraseSelector {
            begin: 0,
            end: 1,
            forward_select: true,
            orig: 0,
            buffer: vec![Symbol::Char(',')],
            selections: vec![],
            breaks: vec![],
        };
        let dict = HashMap::from([(vec![syl![C, E, TONE4]], vec![("測", 100).into()])]);
        sel.init(1, &dict);
    }

    #[test]
    fn should_stop_at_left_boundary() {
        let sel = PhraseSelector {
            begin: 0,
            end: 2,
            forward_select: false,
            orig: 0,
            buffer: vec![
                Symbol::Syllable(syl![C, E, TONE4]),
                Symbol::Syllable(syl![C, E, TONE4]),
            ],
            selections: vec![],
            breaks: vec![],
        };

        assert_eq!(0, sel.after_previous_break_point(0));
        assert_eq!(0, sel.after_previous_break_point(1));
        assert_eq!(0, sel.after_previous_break_point(2));
    }

    #[test]
    fn should_stop_after_first_non_syllable() {
        let sel = PhraseSelector {
            begin: 0,
            end: 2,
            forward_select: false,
            orig: 0,
            buffer: vec![Symbol::Char(','), Symbol::Syllable(syl![C, E, TONE4])],
            selections: vec![],
            breaks: vec![],
        };

        assert_eq!(0, sel.after_previous_break_point(0));
        assert_eq!(1, sel.after_previous_break_point(1));
        assert_eq!(1, sel.after_previous_break_point(2));
    }

    #[test]
    fn should_stop_after_first_selection() {
        let sel = PhraseSelector {
            begin: 0,
            end: 2,
            forward_select: false,
            orig: 0,
            buffer: vec![
                Symbol::Syllable(syl![C, E, TONE4]),
                Symbol::Syllable(syl![C, E, TONE4]),
            ],
            selections: vec![Interval {
                start: 0,
                end: 1,
                is_phrase: true,
                phrase: "冊".to_string(),
            }],
            breaks: vec![],
        };

        assert_eq!(0, sel.after_previous_break_point(0));
        assert_eq!(1, sel.after_previous_break_point(1));
        assert_eq!(1, sel.after_previous_break_point(2));
    }
}