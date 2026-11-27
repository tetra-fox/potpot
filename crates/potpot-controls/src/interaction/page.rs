//! The pages of a panel, one of which is shown at a time.

use super::{Action, Interaction, Target};

impl Interaction {
  /// Returns the page shown, by its place among the panel's pages.
  pub fn page(&self) -> usize {
    self.page
  }

  /// Returns a target, unless it is a control's on a page not shown.
  ///
  /// A target is found in the picture last drawn, which until the next frame is of the page
  /// shown before a switch.
  pub(super) fn reachable(&self, over: Option<Target>) -> Option<Target> {
    over.filter(|over| match *over {
      Target::Control(control) | Target::Readout(control) => {
        self.controls[control].page == self.page
      }
      _ => true,
    })
  }

  /// Shows the page at the place `page`.
  ///
  /// The values of the controls are left as they are.
  pub(super) fn show(&mut self, page: usize, actions: &mut Vec<Action>) {
    assert!(page < self.pages, "the tab is of one of the panel's pages");
    if page != self.page {
      self.page = page;
      actions.push(Action::Redraw);
    }
  }
}
