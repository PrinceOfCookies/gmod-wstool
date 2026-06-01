use super::client;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use steamworks::{AppId, FileType, PublishedFileId, PublishedFileVisibility, UpdateHandle};

pub const VIS_PUBLIC: u8 = 0;
pub const VIS_FRIENDS: u8 = 1;
pub const VIS_PRIVATE: u8 = 2;
pub const VIS_UNLISTED: u8 = 3;

pub fn vis_from_u8(v: u8) -> PublishedFileVisibility {
    match v {
        VIS_FRIENDS => PublishedFileVisibility::FriendsOnly,
        VIS_PRIVATE => PublishedFileVisibility::Private,
        VIS_UNLISTED => PublishedFileVisibility::Unlisted,
        _ => PublishedFileVisibility::Public,
    }
}

pub struct UpdateRequest {
    pub id: u64,
    pub title: Option<String>,
    pub description: Option<String>,
    pub visibility: u8,
    pub tags: Option<Vec<String>>,
    pub content_path: Option<PathBuf>,
    pub preview_path: Option<PathBuf>,
    pub change_note: String,
}

#[derive(Clone)]
pub enum UpdateState {
    Uploading,
    Done { needs_legal_agreement: bool },
    Error(String),
}

pub struct UpdateJob {
    rx: Receiver<UpdateState>,
    state: UpdateState,
    // keep the watch handle alive for the duration of the upload
    _watch: steamworks::UpdateWatchHandle,
}

impl UpdateJob {
    pub fn start(req: UpdateRequest) -> Self {
        let (tx, rx): (Sender<UpdateState>, _) = mpsc::channel();
        let ugc = client().ugc();
        let mut handle: UpdateHandle = ugc.start_item_update(AppId(4000), PublishedFileId(req.id));

        if let Some(t) = &req.title {
            handle = handle.title(t);
        }
        if let Some(d) = &req.description {
            handle = handle.description(d);
        }
        if let Some(tags) = req.tags {
            handle = handle.tags(tags, false);
        }
        if let Some(p) = &req.content_path {
            handle = handle.content_path(p);
        }
        if let Some(p) = &req.preview_path {
            handle = handle.preview_path(p);
        }
        handle = handle.visibility(vis_from_u8(req.visibility));

        let note = req.change_note.clone();
        let note_opt = if note.trim().is_empty() {
            None
        } else {
            Some(note.as_str())
        };

        let watch = handle.submit(note_opt, move |res| {
            let msg = match res {
                Ok((_, needs_legal_agreement)) => UpdateState::Done {
                    needs_legal_agreement,
                },
                Err(e) => UpdateState::Error(e.to_string()),
            };
            let _ = tx.send(msg);
        });

        Self {
            rx,
            state: UpdateState::Uploading,
            _watch: watch,
        }
    }

    // call each frame; returns current state
    pub fn poll(&mut self) -> UpdateState {
        if let Ok(s) = self.rx.try_recv() {
            self.state = s;
        }
        self.state.clone()
    }
}

pub enum CreateState {
    Creating,
    Done { needs_legal_agreement: bool },
    Error(String),
}

pub struct CreateJob {
    rx: Receiver<CreateState>,
    state: CreateState,
}

impl CreateJob {
    pub fn start() -> Self {
        let (tx, rx): (Sender<CreateState>, _) = mpsc::channel();
        client()
            .ugc()
            .create_item(AppId(4000), FileType::Community, move |res| {
                let msg = match res {
                    Ok((_, needs_legal_agreement)) => CreateState::Done {
                        needs_legal_agreement,
                    },
                    Err(e) => CreateState::Error(e.to_string()),
                };
                let _ = tx.send(msg);
            });
        Self {
            rx,
            state: CreateState::Creating,
        }
    }

    pub fn poll(&mut self) -> &CreateState {
        if let Ok(s) = self.rx.try_recv() {
            self.state = s;
        }
        &self.state
    }
}
