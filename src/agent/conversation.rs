use dirs::home_dir;
use std::path::PathBuf;
use std::str::FromStr;

use super::message::Message;
use anyhow::anyhow;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;
use walkdir::WalkDir;

pub(crate) const CONVERSATION_DIR: &str = ".archer/conversations/";

fn get_conversation_dir() -> PathBuf {
    if let Some(conversation_dir) = home_dir().and_then(|x| Some(x.join(CONVERSATION_DIR))) {
        conversation_dir
    } else {
        PathBuf::from(CONVERSATION_DIR)
    }
}

#[derive(Clone)]
pub struct ConversationMetadata {
    path: PathBuf,
    title: String,
}

pub struct ConversationManager {
    pub conversation_files: IndexMap<Uuid, ConversationMetadata>,
    pub active_conversation: usize,
    pub selected_conversation: usize,
}

impl Default for ConversationManager {
    fn default() -> Self {
        // Load existing Conversations
        let mut conversation_files = IndexMap::<Uuid, ConversationMetadata>::new();
        let conversation_dir = get_conversation_dir();
        for entry in WalkDir::new(conversation_dir) {
            if let Some(entry) = entry.ok() {
                if entry.path().is_dir() {
                    continue;
                }

                let path = PathBuf::from(entry.clone().path());
                if let Some(contents) = std::fs::read_to_string(path.clone()).ok() {
                    let convo: Result<Conversation, serde_json::Error> =
                        serde_json::from_str(contents.as_str());
                    if let Some(convo) = convo.ok() {
                        conversation_files.insert(
                            convo.id,
                            ConversationMetadata {
                                path,
                                title: convo.title.unwrap_or(convo.id.to_string()),
                            },
                        );
                    }
                }
            }
        }

        ConversationManager {
            conversation_files,
            active_conversation: 0,
            selected_conversation: 0,
        }
    }
}

impl ConversationManager {
    pub(crate) fn load_conversation(&mut self, id: &Uuid) -> anyhow::Result<Conversation> {
        let file_path = self.get_file_path(id);
        let contents = std::fs::read_to_string(file_path.as_path())?;
        let convo: Conversation = serde_json::from_str(contents.as_str())?;

        anyhow::Ok(convo)
    }

    pub(crate) fn new_conversation(&mut self) -> Conversation {
        let id = Uuid::now_v7();
        let convo = Conversation::new();

        let metadata = ConversationMetadata {
            path: convo.get_file_path(),
            title: id.to_string(),
        };

        self.conversation_files.insert(convo.id, metadata);

        convo
    }

    pub(crate) fn add_conversation(&mut self, conversation: Conversation) {
        let metadata = ConversationMetadata {
            path: conversation.get_file_path(),
            title: conversation.title.unwrap_or(conversation.id.to_string()),
        };
        self.conversation_files.insert(conversation.id, metadata);
    }

    pub(crate) fn get_file_path(&self, id: &Uuid) -> PathBuf {
        let conversation_dir = get_conversation_dir();
        let directory = PathBuf::from(conversation_dir);
        let file_path = directory.join(format!("{}.json", id));
        file_path
    }

    pub(crate) fn load_selected_conversation(&mut self) -> anyhow::Result<Conversation> {
        let ids = self
            .conversation_files
            .keys()
            .into_iter()
            .map(|x| x.clone())
            .collect::<Vec<Uuid>>();

        if let Some(id) = ids.get(self.selected_conversation) {
            self.activate_selected_conversation();
            return self.load_conversation(id);
        } else {
            return Err(anyhow!("Conversation not available"));
        }
    }

    pub(crate) fn set_active_conversation(&mut self, conversation: &Conversation) {
        self.active_conversation = self
            .conversation_files
            .keys()
            .into_iter()
            .position(|x| x == &conversation.id)
            .unwrap();
    }

    pub(crate) fn activate_selected_conversation(&mut self) {
        self.active_conversation = self.selected_conversation;
    }

    pub(crate) fn delete_conversation(&self) -> anyhow::Result<()> {
        todo!();
    }

    pub(crate) fn select_next_conversation(&mut self) {
        if self.selected_conversation < (self.conversation_files.len() - 1) {
            self.selected_conversation += 1;
        }
    }

    pub(crate) fn list_conversations(&self) -> Vec<String> {
        self.conversation_files
            .keys()
            .into_iter()
            .map(|x| x.to_string())
            .collect::<Vec<String>>()
    }

    pub(crate) fn list_titles(&self) -> Vec<String> {
        self.conversation_files
            .values()
            .into_iter()
            .map(|metadata| metadata.title.clone())
            .collect::<Vec<String>>()
    }

    pub(crate) fn select_prev_conversation(&mut self) {
        if self.selected_conversation > 0 {
            self.selected_conversation -= 1;
        }
    }

    pub(crate) fn update_conversation(&mut self, conversation: Conversation) {
        let metadata = ConversationMetadata {
            path: conversation.get_file_path(),
            title: conversation.title.unwrap_or(conversation.id.to_string()),
        };
        *self
            .conversation_files
            .entry(conversation.id)
            .or_insert(metadata.clone()) = metadata.clone();
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Conversation {
    pub id: Uuid,
    pub messages: IndexMap<Uuid, Message>,
    pub selected_message: Option<usize>,
    pub title: Option<String>,
}

impl Conversation {
    pub fn new() -> Self {
        Conversation {
            id: Uuid::now_v7(),
            messages: IndexMap::<Uuid, Message>::new(),
            selected_message: None,
            title: None,
        }
    }
    pub fn get_file_path(&self) -> PathBuf {
        let conversation_dir = get_conversation_dir();
        let directory = PathBuf::from(conversation_dir);
        let file_path = directory.join(format!("{}.json", self.id));
        file_path
    }

    pub fn generate_message_id(&self) -> Uuid {
        Uuid::new_v4()
    }

    pub fn add_message(&mut self, id: Uuid, message: Message) {
        self.messages.insert(id, message);
        self.select_last_message();
    }

    pub(crate) fn save(&self) -> anyhow::Result<()> {
        let conversation_dir = get_conversation_dir();
        let data = serde_json::to_string(self)?;
        let file_path = self.get_file_path();
        let directory = PathBuf::from(conversation_dir);

        tokio::spawn(async move {
            tokio::fs::create_dir_all(directory).await?;
            let mut file = File::create(file_path).await?;
            file.write_all(data.as_bytes()).await?;
            anyhow::Ok(())
        });

        anyhow::Ok(())
    }
    pub fn delete_selected_message(&mut self) {
        if let Some(Some(uuid)) = self.selected_message.map(|idx| self.get_uuid_by_index(idx)) {
            self.messages.remove(&uuid);
            self.select_prev_message();
        }
    }

    pub fn get_selected_uuid(&self) -> Option<Uuid> {
        if let Some(selected_id) = self.selected_message {
            self.get_uuid_by_index(selected_id)
        } else {
            None
        }
    }

    pub fn get_position(&self) -> (usize, usize) {
        (self.messages.len(), self.selected_message.unwrap_or(0))
    }

    pub fn get_uuid_by_index(&self, id: usize) -> Option<Uuid> {
        Vec::from_iter(self.messages.keys()).get(id).map(|x| **x)
    }

    pub fn select_last_message(&mut self) {
        self.selected_message = Some(self.messages.len() - 1);
    }

    pub fn replace_message(&mut self, id: Uuid, message: Message) {
        *self.messages.get_mut(&id).unwrap() = message;
    }

    pub fn get_selected_message(&self) -> anyhow::Result<Message> {
        if let Some(Some(uuid)) = self.selected_message.map(|idx| self.get_uuid_by_index(idx)) {
            if let Some(message) = self.messages.get(&uuid) {
                return anyhow::Ok(message.clone());
            }
        }
        return Err(anyhow!("Could not retrieve message"));
    }

    pub fn select_next_message(&mut self) {
        if let Some(currently_selected) = self.selected_message {
            if currently_selected < (self.messages.len() - 1) {
                self.selected_message = Some(currently_selected + 1);
            }
        } else {
            self.selected_message = Some(0);
        }
    }

    pub fn select_prev_message(&mut self) {
        if let Some(currently_selected) = self.selected_message {
            if currently_selected > 0 {
                self.selected_message = Some(currently_selected - 1);
            }
        } else {
            self.selected_message = Some(0);
        }
    }
}
