//! Browser host adapter: the editor retains ownership of its native document format.
use sovereign_cloud_ui::{AppSpec, DocumentState, Editor, Snapshot};

pub struct Adapter<'a>(pub &'a mut deckcraft_ui_egui::SlideApp);
impl Editor for Adapter<'_> {
    fn document(&self) -> Option<DocumentState> {
        self.0.session.active().map(|d| DocumentState { id: d.uid, revision: d.revision, name: d.title(), dirty: d.is_dirty() })
    }
    fn capture(&self) -> Result<Snapshot, String> {
        let document = self.document().ok_or("No presentation is open")?;
        let doc = self.0.session.active().ok_or("No presentation is open")?.doc.clone();
        Ok(Snapshot {
            document,
            encode: Box::new(move || deckcraft_engine::cmd::file::save_bytes(&doc, AppSpec::DECK.extension).map_err(|e| e.to_string())),
        })
    }
    fn saved(&mut self, id: u64, revision: u64, _bytes: &[u8]) -> Result<Option<u64>, String> {
        // Only the active, unchanged snapshot is marked clean. Other tabs retain their edits.
        if self.document().is_some_and(|d| d.id == id && d.revision == revision) {
            deckcraft_engine::cmd::file::mark_saved(&mut self.0.session, None).map_err(|e| e.to_string())?;
            return Ok(self.document().map(|d| d.revision));
        }
        Ok(None)
    }
    fn open(&mut self, name: &str, bytes: &[u8]) -> Result<DocumentState, String> {
        let doc = deckcraft_engine::cmd::file::open_presentation(name, bytes).map_err(|e| e.to_string())?;
        deckcraft_engine::cmd::file::add_opened(&mut self.0.session, name, None, doc);
        self.document().ok_or_else(|| "Could not open the presentation".into())
    }
}
