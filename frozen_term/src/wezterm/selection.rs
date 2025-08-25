use std::ops::{Range, RangeInclusive};

use wezterm_term::PhysRowIndex;

use crate::terminal_grid::VisiblePosition;

#[derive(Debug, Clone, PartialEq)]
pub struct SelectionPosition {
    pub x: usize,
    pub y: usize,
}

impl SelectionPosition {
    pub fn from_visible(visible: VisiblePosition, offset: usize) -> Self {
        Self {
            x: visible.x,
            y: visible.y + offset,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectionState {
    step: SelectionStep,
    scroll_offset: usize,
}

pub struct Selection {
    start: SelectionPosition,
    end: SelectionPosition,
}

#[derive(Debug, Clone, PartialEq)]
enum SelectionStep {
    None,
    Starting(SelectionPosition),
    Selecting {
        start: SelectionPosition,
        end: VisiblePosition,
    },
    Selected {
        start: SelectionPosition,
        end: SelectionPosition,
    },
}

impl SelectionState {
    pub(crate) fn new() -> Self {
        Self {
            step: SelectionStep::None,
            scroll_offset: 0,
        }
    }

    #[must_use]
    pub fn start(&mut self, pos: VisiblePosition) -> Option<Range<PhysRowIndex>> {
        let invalidate = match &self.step {
            SelectionStep::Selecting { start, end } => {
                let end = SelectionPosition::from_visible(end.clone(), self.scroll_offset);
                if start.y > end.y {
                    Some(end.y..start.y + 1)
                } else {
                    Some(start.y..end.y + 1)
                }
            }
            SelectionStep::Selected { start, end } => {
                if start.y > end.y {
                    Some(end.y..start.y + 1)
                } else {
                    Some(start.y..end.y + 1)
                }
            }
            _ => None,
        };
        self.step =
            SelectionStep::Starting(SelectionPosition::from_visible(pos, self.scroll_offset));
        invalidate
    }

    #[must_use]
    pub fn move_end(&mut self, pos: VisiblePosition) -> Option<Range<PhysRowIndex>> {
        match &self.step {
            SelectionStep::Selecting { start, .. } | SelectionStep::Starting(start) => {
                let old_line = match &self.step {
                    SelectionStep::Selecting { end, .. } => {
                        SelectionPosition::from_visible(end.clone(), self.scroll_offset).y
                    }
                    SelectionStep::Starting(start) => start.y,
                    _ => unreachable!(),
                };

                let new_line = SelectionPosition::from_visible(pos.clone(), self.scroll_offset).y;

                let range = if new_line > old_line {
                    old_line..new_line + 1
                } else {
                    new_line..old_line + 1
                };

                self.step = SelectionStep::Selecting {
                    start: start.clone(),
                    end: pos,
                };
                Some(range)
            }
            _ => None,
        }
    }

    pub fn finish(&mut self) {
        match &self.step {
            SelectionStep::Selecting { start, end } => {
                let end = SelectionPosition::from_visible(end.clone(), self.scroll_offset);
                // let start_y = start.y;
                // let end_y = end.y;
                self.step = SelectionStep::Selected {
                    start: start.clone(),
                    end,
                };
                // if start_y > end_y {
                //     Some(end_y..start_y + 1)
                // } else {
                //     Some(start_y..end_y + 1)
                // }
            }
            SelectionStep::Starting(_) => {
                self.step = SelectionStep::None;
            }
            _ => (),
        }
    }

    #[must_use]
    pub fn set_scroll(&mut self, offset: usize) -> Option<Range<PhysRowIndex>> {
        let range = match &self.step {
            SelectionStep::Selecting { end, .. } => {
                let old = SelectionPosition::from_visible(end.clone(), self.scroll_offset).y;
                let new = SelectionPosition::from_visible(end.clone(), offset).y;

                if old == new {
                    None
                } else if old > new {
                    Some(new..old + 1)
                } else {
                    Some(old..new + 1)
                }
            }
            _ => None,
        };
        self.scroll_offset = offset;
        range
    }

    pub fn is_active(&self) -> bool {
        match &self.step {
            SelectionStep::Selecting { .. } => true,
            SelectionStep::Starting(_) => true,
            _ => false,
        }
    }

    pub fn get_selection(&self) -> Option<Selection> {
        match &self.step {
            SelectionStep::Selecting { start, end } => Some(Selection::new(
                start.clone(),
                SelectionPosition::from_visible(end.clone(), self.scroll_offset),
            )),
            SelectionStep::Selected { start, end } => {
                Some(Selection::new(start.clone(), end.clone()))
            }
            _ => None,
        }
    }
}

impl Selection {
    fn new(start: SelectionPosition, end: SelectionPosition) -> Self {
        let (start, end) = if start.y < end.y || (start.y == end.y && start.x <= end.x) {
            (start, end)
        } else {
            (end, start)
        };

        Self { start, end }
    }
}

pub fn is_selected(selection: &Option<Selection>, pos: SelectionPosition) -> bool {
    let Some(selection) = selection else {
        return false;
    };

    // Check if position is within selection
    if pos.y < selection.start.y || pos.y > selection.end.y {
        return false;
    }

    if pos.y == selection.start.y && pos.y == selection.end.y {
        // Selection is on single line
        pos.x >= selection.start.x && pos.x <= selection.end.x
    } else if pos.y == selection.start.y {
        // First line of multi-line selection
        pos.x >= selection.start.x
    } else if pos.y == selection.end.y {
        // Last line of multi-line selection
        pos.x <= selection.end.x
    } else {
        // Middle line of multi-line selection
        true
    }
}
