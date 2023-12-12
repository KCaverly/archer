use super::message::Message;
use anyhow::anyhow;
use indexmap::IndexMap;
use uuid::Uuid;

#[derive(Default)]
pub struct Conversation {
    pub messages: IndexMap<Uuid, Message>,
    pub selected_message: Option<usize>,
}

impl Conversation {
    pub fn new() -> Self {
        Conversation {
            messages: IndexMap::<Uuid, Message>::new(),
            selected_message: None,
        }
    }

    pub fn generate_message_id(&self) -> Uuid {
        Uuid::new_v4()
    }

    pub fn add_message(&mut self, id: Uuid, message: Message) {
        self.messages.insert(id, message);
        self.select_last_message();
    }

    pub fn delete_selected_message(&mut self) {
        if let Some(Some(uuid)) = self.selected_message.map(|idx| self.get_uuid_by_index(idx)) {
            self.messages.remove(&uuid);
            self.select_prev_message();
        }
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

    pub fn unfocus(&mut self) {
        // We are no longer changing which note is selected when we focus
    }

    pub fn focus(&mut self) {
        // No longer change which note is selected when we focus
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
            let next_selected = currently_selected + 1;
            if next_selected < self.messages.len() {
                self.selected_message = Some(next_selected);
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
