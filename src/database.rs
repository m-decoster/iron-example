use model::Post;

#[derive(Clone, Debug)]
pub struct Database {
    posts: Vec<Post>,
}

impl Database {
    pub fn new() -> Database {
        Database { posts: vec![] }
    }

    pub fn add_post(&mut self, post: Post) {
        self.posts.push(post);
    }

    pub fn posts(&self) -> &Vec<Post> {
        &self.posts
    }
}
