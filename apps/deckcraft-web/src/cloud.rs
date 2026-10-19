//! Browser host adapter: the editor retains ownership of its native document format.
use sovereign_cloud_ui::{AppSpec, DocumentState, Editor, Snapshot};

pub struct Adapter<'a>(pub &'a mut deckcraft_ui_egui::SlideApp);
impl Editor for Adapter<'_> {
    fn create_document(&mut self) -> Result<(), String> {
        if self.document().is_some() {
            return Ok(());
        }
        self.0.session.execute("file.new", &serde_json::json!({})).map_err(|e| e.to_string())?;
        Ok(())
    }
    fn set_user_profile(&mut self, profile: Option<sovereign_cloud_ui::UserProfile<'_>>) {
        let previous = self.0.session.prefs.user_id.is_some();
        self.0.session.prefs.user_id = profile.map(|p| p.id.to_owned());
        if let Some(profile) = profile {
            if self.0.session.prefs.author != profile.display_name {
                self.0.session.prefs.author = profile.display_name.to_owned();
            }
        } else if previous {
            self.0.session.prefs.author = "Presenter".into();
        }
    }

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

#[cfg(test)]
mod identity_tests {
    use super::*;

    #[test]
    fn account_profile_follows_host_and_cannot_restore_identity() {
        let mut app = deckcraft_ui_egui::SlideApp::new(deckcraft_engine::Session::new(), Default::default());
        app.session.prefs.author = "Same display name".into();
        for id in [None, Some("account-a"), Some("account-a"), Some("account-b"), None] {
            Adapter(&mut app).set_user_profile(id.map(|id| sovereign_cloud_ui::UserProfile { id, display_name: "Subnet name" }));
            assert_eq!(app.session.prefs.user_id.as_deref(), id);
            if id.is_some() {
                assert_eq!(app.session.prefs.author, "Subnet name");
            }
        }
        Adapter(&mut app).set_user_profile(Some(sovereign_cloud_ui::UserProfile { id: "account-a", display_name: "Subnet name" }));
        let saved = serde_json::to_value(&app.session.prefs).unwrap();
        assert!(saved.get("user_id").is_none() && saved.get("userId").is_none());
        let forged = serde_json::json!({"user_id": "other-account", "userId": "other-account"});
        let restored: deckcraft_engine::Prefs = serde_json::from_value(forged).unwrap();
        assert!(restored.user_id.is_none());
    }
}

#[cfg(test)]
mod creation_tests {
    use super::*;
    #[test]
    fn workspace_creation_opens_one_native_document_and_preserves_it_on_repeat() {
        let mut app = deckcraft_ui_egui::SlideApp::new(deckcraft_engine::Session::new(), Default::default());
        let mut adapter = Adapter(&mut app);
        adapter.create_document().unwrap();
        let first = adapter.document().unwrap();
        adapter.create_document().unwrap();
        assert_eq!(adapter.document().unwrap().id, first.id);
        assert!(!(adapter.capture().unwrap().encode)().unwrap().is_empty());
    }
}
