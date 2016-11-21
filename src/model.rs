use chrono::datetime::DateTime;
use chrono::offset::utc::UTC;
use uuid::Uuid;

#[derive(Clone, Debug, RustcEncodable, RustcDecodable)]
pub struct Post {
    summary: String,
    contents: String,
    author_handle: String,
    date_time: DateTime<UTC>,
    uuid: Uuid,
}

impl Post {
    pub fn new(summary: &str,
               contents: &str,
               author: &Author,
               date_time: DateTime<UTC>,
               uuid: Uuid)
               -> Post {
        Post {
            summary: summary.to_string(),
            contents: contents.to_string(),
            author_handle: author.handle.clone(),
            date_time: date_time,
            uuid: uuid,
        }
    }

    pub fn uuid(&self) -> &Uuid {
        &self.uuid
    }
}

#[derive(Clone, Debug, RustcEncodable, RustcDecodable)]
pub struct Author {
    handle: String,
}

impl Author {
    pub fn new(handle: &str) -> Author {
        Author { handle: handle.to_string() }
    }
}
