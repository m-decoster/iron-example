use std::sync::{Arc, Mutex};
use std::io::Read;
use iron::{Handler, status, IronResult, Response, Request, AfterMiddleware};
use iron::headers::ContentType;
use rustc_serialize::json;
use database::Database;
use uuid::Uuid;
use router::Router;
use model::Post;
use std::error::Error;

/// Match a `Result` into its inner value or
/// return `500 Internal Server Error`,
/// or some other provided error using the second variant of this macro.
macro_rules! try_handler {
    ( $e:expr ) => {
        match $e {
            Ok(x) => x,
            Err(e) => return Ok(Response::with((status::InternalServerError, e.description())))
        }
    };
    ( $e:expr, $error:expr ) => {
        match $e {
            Ok(x) => x,
            Err(e) => return Ok(Response::with(($error, e.description())))
        }
    }
}

/// Lock a `Mutex`. This macro simply calls `m.lock().unwrap()`,
/// because the thread should panic if the lock can not be obtained:
/// we cannot recover from that.
macro_rules! lock {
    ( $e:expr ) => { $e.lock().unwrap() }
}


/// Get the value of a parameter in the URI.
/// If the parameter was absent, return `400 Bad Request`.
/// If we could not obtain the parameter list, return `500 Internal Server Error`.
macro_rules! get_http_param {
    ( $r:expr, $e:expr ) => {
        match $r.extensions.get::<Router>() {
            Some(router) => {
                match router.find($e) {
                    Some(val) => val,
                    None => return Ok(Response::with(status::BadRequest)),
                }
            }
            None => return Ok(Response::with(status::InternalServerError)),
        }
    }
}

pub struct Handlers {
    pub feed: FeedHandler,
    pub make_post: MakePostHandler,
    pub post: PostHandler,
}

impl Handlers {
    pub fn new(database: Database) -> Handlers {
        let database = Arc::new(Mutex::new(database));
        Handlers {
            feed: FeedHandler::new(database.clone()),
            make_post: MakePostHandler::new(database.clone()),
            post: PostHandler::new(database.clone()),
        }
    }
}

pub struct FeedHandler {
    database: Arc<Mutex<Database>>,
}

impl FeedHandler {
    fn new(database: Arc<Mutex<Database>>) -> FeedHandler {
        FeedHandler { database: database }
    }
}

impl Handler for FeedHandler {
    fn handle(&self, _: &mut Request) -> IronResult<Response> {
        let payload = try_handler!(json::encode(lock!(self.database).posts()));
        Ok(Response::with((status::Ok, payload)))
    }
}

pub struct MakePostHandler {
    database: Arc<Mutex<Database>>,
}

impl MakePostHandler {
    fn new(database: Arc<Mutex<Database>>) -> MakePostHandler {
        MakePostHandler { database: database }
    }
}

impl Handler for MakePostHandler {
    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        let mut payload = String::new();
        try_handler!(req.body.read_to_string(&mut payload));

        let post = try_handler!(json::decode(&payload), status::BadRequest);

        lock!(self.database).add_post(post);

        Ok(Response::with((status::Created, payload)))
    }
}

pub struct PostHandler {
    database: Arc<Mutex<Database>>,
}

impl PostHandler {
    fn new(database: Arc<Mutex<Database>>) -> PostHandler {
        PostHandler { database: database }
    }

    fn find_post(&self, id: &Uuid) -> Option<Post> {
        let locked = lock!(self.database);
        let mut iterator = locked.posts().iter();
        iterator.find(|post| post.uuid() == id).map(|post| post.clone())
    }
}

impl Handler for PostHandler {
    fn handle(&self, req: &mut Request) -> IronResult<Response> {
        let ref post_id = get_http_param!(req, "id");

        let id = try_handler!(Uuid::parse_str(post_id), status::BadRequest);

        if let Some(post) = self.find_post(&id) {
            let payload = try_handler!(json::encode(&post), status::BadRequest);
            Ok(Response::with((status::Ok, payload)))
        } else {
            Ok(Response::with((status::NotFound)))
        }
    }
}

pub struct JsonAfterMiddleware;

impl AfterMiddleware for JsonAfterMiddleware {
    fn after(&self, _: &mut Request, mut res: Response) -> IronResult<Response> {
        res.headers.set(ContentType::json());
        Ok(res)
    }
}
