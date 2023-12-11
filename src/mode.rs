use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Viewer,
    Input,
    ActiveInput,
    ActiveViewer,
    ModelSelector,
    MessageViewer,
}
