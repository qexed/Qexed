use qexed_doc_macros::DocValue;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, DocValue)]

pub enum MODE {
    HOUR,
    DAY,
    MONTH,
}
impl Default for MODE {
    fn default() -> Self {
        Self::DAY
    }
}

impl MODE {
    pub fn as_tklog(self) -> tklog::MODE {
        match self {
            Self::HOUR  =>tklog::MODE::HOUR ,
            Self::DAY  => tklog::MODE::DAY  ,
            Self::MONTH =>tklog::MODE::MONTH,
        }
    }
}