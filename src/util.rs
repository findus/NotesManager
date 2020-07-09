use std::path::{Path, PathBuf};
use note::{NotesMetadata, HeaderParser};
use profile;
use uuid::Uuid;

use chrono::{Utc};

pub fn get_hash_path(path: &Path) -> PathBuf {
    let folder = path.parent().unwrap().to_string_lossy().into_owned();
    let new_file_name = format!(".{}_hash",path.file_name().unwrap().to_string_lossy().into_owned());

    let metadata_file_path = format!("{}/{}",&folder,&new_file_name).to_owned();
    std::path::Path::new(&metadata_file_path).to_owned()
}

pub fn get_notes_file_from_metadata(metadata: &NotesMetadata) -> PathBuf {
    let path = format!("{}/{}/{}", profile::get_notes_dir(), metadata.subfolder, metadata.subject_with_identifier());
    Path::new(&path).to_path_buf()
}

pub fn generate_uuid() -> String {
    Uuid::new_v4().to_string()
}

/**
From:
X-Uniform-Type-Identifier
Content-Type

**/

pub struct HeaderBuilder {
     headers: Vec<(String,String)>
}

impl HeaderBuilder {
    pub fn new() -> HeaderBuilder {
        let mut headers: Vec<(String,String)> = vec![];
        headers.push(("Content-Type".to_string(), "text/html;\ncharset=utf-8".to_string()));
        headers.push(("Content-Transfer-Encoding".to_string(), "quoted-printable".to_string()));
        headers.push(("Mime-Version".to_string(), "1.0 (Mac OS X Notes 4.6 \\(879.10\\))".to_string()));
        let date = Utc::now().to_rfc2822();
        headers.push(("Date".to_string(), date.clone()));
        headers.push(("X-Mail-Created-Date".to_string(), date.clone()));
        headers.push(("X-Universally-Unique-Identifier".to_string(), generate_uuid()));
        //TODO read mail from settings or pass them as arg
        headers.push(("Message-Id".to_string(), format!("<{}@f1ndus.de>", generate_uuid())));

        HeaderBuilder {
            headers
        }
    }

    pub fn with_subject(mut self, subject: String) -> Self {
        self.headers.push(("Subject".to_string(), subject));
        self
    }

    pub fn build(self) -> Vec<(String, String)> {
        self.headers
    }
}

pub fn generate_mail_headers(subject: String) -> Vec<(String, String)> {
    HeaderBuilder::new().with_subject(subject).build()
}