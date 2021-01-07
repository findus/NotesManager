extern crate imap;
extern crate native_tls;
extern crate mailparse;
extern crate log;
extern crate regex;
extern crate jfs;
extern crate serde_derive;
extern crate serde_json;
extern crate serde;


use self::log::{info, warn, debug};
use self::imap::Session;
use std::net::TcpStream;
use self::native_tls::TlsStream;
use self::imap::types::{Fetch};
use model::{NotesMetadata};
use note::{RemoteNoteHeaderCollection, RemoteNoteMetaData, LocalNote, IdentifyableNote};

use ::{profile};
use imap::error::Error;
use converter::convert_to_html;
use imap::types::Mailbox;
#[cfg(test)]
extern crate mockall;
#[cfg(test)]
use mockall::{automock, predicate::*};

pub trait ImapSession<S> {

}

pub struct TlsImapSession {
    session: Session<TlsStream<TcpStream>>
}

impl TlsImapSession {
    fn login() -> Session<TlsStream<TcpStream>> {
        let profile = self::profile::load_profile();

        let domain = profile.imap_server.as_str();
        info!("Connecting to {}", domain);

        let tls = native_tls::TlsConnector::builder().build().unwrap();

        // we pass in the domain twice to check that the server's TLS
        // certificate is valid for the domain we're connecting to.
        let client = imap::connect((domain, 993), domain, &tls).unwrap();

        // the client we have here is unauthenticated.
        // to do anything useful with the e-mails, we need to log in
        let imap_session = client
            .login(profile.username, profile.password)
            .map_err(|e| e.0);

        return imap_session.unwrap();
    }
}

impl ImapSession<Session<TlsStream<TcpStream>>> for TlsImapSession {

}

#[cfg_attr(test, automock)]
pub trait MailService<T: 'static> {
    /// Iterates through all Note-Imap folders and fetches the mail header content plus
    /// the folder name.
    ///
    /// The generated dataset can be used to check for duplicated notes that needs
    /// to be merged
    fn fetch_headers(&mut self) -> Result<RemoteNoteHeaderCollection,Error>;
    /// Creates a new Subfolder for storing notes
    fn create_mailbox(&mut self, note: &NotesMetadata) -> Result<(), Error>;
    /// Fetches the actual content from a note
    fn fetch_note_content(&mut self, subfolder: &str, uid: i64) -> Result<String, Error>;
    /// Exposes the active imap connection
    fn get_session(&self) -> T;
    /// Updates a local message, either if it got updated or if it is a new localnote
    /// This App should only support "merged" notes, notes that only have one body.
    ///
    /// If the passed localnote has >1 bodies it will reject it.
    fn update_message(&mut self, localnote: &LocalNote) -> Result<u32, Error>;
    /// Selects a specific subfolder
    fn select(&mut self, folder: &str) -> Result<Mailbox, Error>;
}

pub struct MailServiceImpl {
    session: TlsImapSession
}

impl MailServiceImpl {
    pub fn new_with_login() -> MailServiceImpl {
        MailServiceImpl {
            session: TlsImapSession {
                session: TlsImapSession::login()
            }
        }
    }

    pub fn fetch_headers_in_folder(&mut self, folder_name: String) -> Vec<RemoteNoteMetaData> {
        if let Some(result) = self.session.session.select(&folder_name).err() {
            warn!("Could not select folder {} [{}]", &folder_name, result)
        }
        let messages_result = self.session.session.fetch("1:*", "(RFC822.HEADER UID)");
        match messages_result {
            Ok(messages) => {
                debug!("Message Loading for {} successful", &folder_name.to_string());
                messages.iter().map( |fetch|{
                    self.get_headers(fetch, folder_name.clone())
                }).collect()
            },
            Err(error) => {
                warn!("Could not load notes from {}! Does this Folder contains any messages? {}", &folder_name.to_string(), error);
                Vec::new()
            }
        }
    }

    /**
    Returns empty vector if something fails
    */
    fn get_headers(&mut self,fetch: &Fetch, foldername: String) -> RemoteNoteMetaData {
        match mailparse::parse_headers(fetch.header().unwrap()) {
            Ok((header, _)) => {
                let  headers = header.into_iter().map(|h| (h.get_key().unwrap(), h.get_value().unwrap())).collect();
                RemoteNoteMetaData {
                    headers,
                    folder: foldername.to_string(),
                    uid: fetch.uid.unwrap() as i64,
                }
            },
            _ => panic!("No Headers presentfor fetch with uid {}", fetch.uid.unwrap())
        }
    }

    fn get_body(&mut self,fetch: &Fetch) -> Option<String> {
        match mailparse::parse_mail(fetch.body()?) {
            Ok(body) => body.get_body().ok(),
            _ => None
        }
    }

    pub fn list_note_folders(&mut self) -> Result<Vec<String>,imap::error::Error> {
        let folders_result = self.session.session.list(None, Some("Notes*"));
        match folders_result {
            Ok(result) => {
                let names: Vec<String> = result.iter().map(|name| name.name().to_string()).collect();
                Ok(names)
            }
            Err(e) => Err(e)
        }
    }

    /// Deletes all notes remotely that have the uuid provided by local_note, expect
    /// the note with uid_to_keep
    fn delete_old_mergeable_notes(&mut self,
                                  local_note: &LocalNote,
                                  uid_to_keep: u32) -> Result<(),Error>
    {
        self.session.session.
            select(&local_note.metadata.folder())
            .and_then(|_| self.session.session.uid_search(
                format!("HEADER X-Universally-Unique-Identifier {}", local_note.body[0].message_id)))
            .and_then(|uids| {
                let uids: Vec<String> = uids.into_iter()
                    .filter(|uid| uid != &uid_to_keep )
                    .map(|x| (x.to_string())).collect();
                for uid in uids {
                    info!("Will delete remote note with uid: {}", uid);
                    self.flag_as_deleted(uid)?;
                }
                self.delete_flagged()?;
                Ok(())
            })
    }

    fn delete_flagged(&mut self) -> imap::error::Result<Vec<u32>> {
        self.session.session.expunge()
    }

    fn flag_as_deleted(&mut self, uid: String) -> imap::error::Result<()> {
        // If note was new everything is ready
        self.session.session.uid_store(uid, "+FLAGS.SILENT (\\Seen \\Deleted)".to_string())?;
        Ok(())
    }
}

impl MailService<Session<TlsStream<TcpStream>>> for MailServiceImpl {

    fn fetch_headers(&mut self) -> Result<Vec<RemoteNoteMetaData>,Error> {
        info!("Fetching Headers of Remote Notes...");
        let folders = self.list_note_folders()?;
        let header = folders.iter().map(|folder_name| {
            self.fetch_headers_in_folder(folder_name.to_string())
        })
            .flatten()
            .collect();
        Ok(header)
    }

    fn create_mailbox(&mut self, note: &NotesMetadata) -> Result<(), Error> {
        self.session.session.create(&note.subfolder).or(Ok(()))
    }

    fn fetch_note_content(&mut self, subfolder: &str, uid: i64) -> Result<String, Error> {
        if let Some(result) = self.session.session.select(&subfolder).err() {
            warn!("Could not select folder {} [{}]", &subfolder, result)
        }

        let messages_result = self.session.session.uid_fetch(uid.to_string(), "(RFC822 UID)");
        match messages_result {
            Ok(message) => {
                debug!("Message Loading for message with UID {} successful", uid);
                let first_message = message.first().expect("Expected message");
                Ok(self.get_body(first_message).expect("Expected note body, found none"))
            },
            Err(error) => {
                warn!("Could not load notes from {}! {}", &subfolder, error);
                Err(error)
            }
        }
    }

    fn get_session(&self) -> Session<TlsStream<TcpStream>> {
        unimplemented!()
    }

    fn update_message(&mut self, localnote: &LocalNote) -> Result<u32, Error> {
        //Todo check >1

        let headers = localnote.to_header_vector().iter().map( |(k,v)| {
            format!("{}: {}",k,v)
        })
            .collect::<Vec<String>>()
            .join("\n");

        // Updated message must be merged
        //let _content = converter::convert_to_html(&localnote.body.first().unwrap());

        let body = localnote.body.first().unwrap();
        let message = format!("{}\n\n{}",headers, convert_to_html(body));

        self.session.session
            // Write new message into the mailbox
            .append(&localnote.metadata.subfolder, message.as_bytes())
            // Select the appropriate mailbox, in which the updated message was saved
            .and_then(|_| self.session.session.select(&localnote.metadata.subfolder))
            // Set the old (overridden) message to "deleted", so that it can be expunged
            .and_then(|_| {
                if localnote.metadata.new == false {
                    self.flag_as_deleted(localnote.body.first().unwrap().uid.unwrap().to_string())
                } else {
                    Ok(())
                }
            })
            // Expunge them //TODO might need check if note is new, skip if note is new
            .and_then(|_| self.delete_flagged())
            // Search for the new message, to get the new UID of the updated message
            .and_then(|_| self.session.session.uid_search(format!("HEADER Message-ID {}", localnote.body[0].message_id)))
            // Get the first UID
            .and_then(|id| id.into_iter().collect::<Vec<u32>>().first().cloned().ok_or(imap::error::Error::Bad("no uid found".to_string())))
            // Save the new UID to the metadata file, also set seen flag so that mail clients dont get notified on updated message
            .and_then(|new_uid| self.session.session.uid_store(format!("{}", &new_uid), "+FLAGS.SILENT (\\Seen)".to_string()).map(|_| new_uid))
            // Delete dangling remote non merged notes
            .and_then(|new_uid| self.delete_old_mergeable_notes(&localnote, new_uid).map(|_| new_uid))
    }

    fn select(&mut self, folder: &str) -> Result<Mailbox, Error> {
        //todo wrap mailbox type?
        self.session.session.select(folder)
    }
}