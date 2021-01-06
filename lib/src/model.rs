use note::{NoteHeaders, HeaderParser, RemoteNoteMetaData};
use ::{util, profile};
use schema::metadata;
use schema::body;
use note::LocalNote;
use std::str::Lines;

#[derive(Identifiable,Clone,Queryable,Insertable,Debug,Eq)]
#[table_name="metadata"]
#[primary_key(uuid)]
pub struct NotesMetadata {
    /// Stores the subfolder name of the folder in which
    /// the note is saved
    pub subfolder: String,
    pub locally_deleted: bool,
    pub locally_edited: bool,
    /// Indicator for newly created notes, so that they
    /// dont get deleted while syncing
    pub new: bool,
    pub date: String, //TODO type
    /// UUID for the message. This uuid never changes after
    /// creating a note.
    ///
    /// However multiple notes with the name can exist remotely
    /// if notes are getting edited simultaneously on multiple
    /// devices, the notes app recognizes this and duplicates
    /// the note the first with the content that was edited on
    /// device1, and the second with the content that was
    /// edited on device2.
    pub uuid: String,
    pub mime_version: String,
}

impl NotesMetadata {
    pub fn new(header: &NoteHeaders, subfolder: String) -> Self {
        NotesMetadata {
            subfolder,
            locally_deleted: false,
            locally_edited: false,
            new: false,
            date: header.date(),
            uuid: header.uuid(),
            mime_version: header.mime_version(),
        }
    }

    pub fn from_remote_metadata(remote_metadata: &RemoteNoteMetaData) -> Self {
        NotesMetadata {
            subfolder: remote_metadata.folder.clone(),
            locally_deleted: false,
            locally_edited: false,
            new: false,
            date: remote_metadata.headers.date(),
            uuid: remote_metadata.headers.uuid(),
            mime_version: remote_metadata.headers.mime_version()
        }
    }
}

#[derive(Identifiable,Clone,Queryable,Insertable,Associations,Debug,Eq)]
#[table_name="body"]
#[belongs_to(NotesMetadata, foreign_key="metadata_uuid")]
#[primary_key(message_id)]
pub struct Body {
    /// Stores old message-id after editing
    /// the note. If the notes are getting synced
    /// this is neede to check if the remote note
    /// also changed, if this is the case
    pub old_remote_message_id: Option<String>,
    /// Identifier for a note in a certain state. This
    /// ID changes every time the note gets edited.
    ///
    /// If you sync the notes and the remote message-id
    /// changed it is likely that the note got edited
    /// on another device.
    pub message_id: String,
    pub text: Option<String>,
    /// The IMAP UID identifier
    pub uid: Option<i64>,
    /// Foreign key to a Metadata Object, every Metadata
    /// Object can have n Bodies
    pub metadata_uuid: String
}

impl Body {
    pub fn new(uid: Option<i64>, metadata_reference: String) -> Body {
        let profile = profile::load_profile();
        Body {
            old_remote_message_id: None,
            message_id: format!("<{}@{}", util::generate_uuid(), profile.domain()),
            text: None,
            uid,
            metadata_uuid: metadata_reference
        }
    }

    pub fn subject(&self) -> String {
        let str = "".to_string();
        let x = self.text.as_ref().unwrap_or(&str);
        let mut lines = x.lines();
        let first_line = lines.next();
        first_line.unwrap_or("").to_string()
    }

    pub fn subject_with_identifier(&self) -> String {
        if self.uid.is_none() {
            format!("{}_{}","new", self.subject_escaped())
        } else {
            format!("{}_{}", self.uid.unwrap(), self.subject_escaped())
        }
    }

    ///
   /// Prints an espaced subject, removes any character that might cause problems when
   /// writing files to disk
   ///
   /// Every Filename should include the title of the note, only saving the file with the uuid
   /// would be quite uncomfortable, with the title, the user has a tool to quickly skim or
   /// search through the notes with only using the terminal or explorer.
   ///
    pub fn subject_escaped(&self) -> String {
        let regex = regex::Regex::new(r#"[.<>:\\"/\|?*]"#).unwrap();
        let escaped_string = format!("{}", self.subject())
            .replace("/", "_").replace(" ", "_");
        // .replace(|c: char| !c.is_ascii(), "");
        regex.replace_all(&escaped_string, "").into_owned()
    }

    #[cfg(test)]
    pub fn is_inside_localnote(&self, local_note: &LocalNote) -> bool {
        if local_note.metadata.uuid == self.metadata_uuid {
            return local_note.body.iter().filter(|e| e == &self).collect::<Vec<_>>().len() == 1
        } else {
            return false
        }
    }
}