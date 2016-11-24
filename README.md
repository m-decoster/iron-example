# Creating a web API using Iron

In this tutorial, we will be looking at how to use [Iron](http://ironframework.io) to create a web API.  
We will be creating an API for a small public Twitter-like application, where anyone can create new posts,
posts can be opened individually or shown in a feed chronologically.

This tutorial assumes familiarity with Rust, Rust macros and basic knowledge of how HTTP works.

This tutorial starts from a complete project and explains the different parts
of the project. After finishing this tutorial, you should have a basic understanding of how
an Iron application is constructed and how to create your own, more complicated, web applications. 

## Getting started

You should start now by creating a new binary project:

```
cargo new --bin mywebapi
```

## Cargo dependencies

You should have following dependencies in your `Cargo.toml` file:

```
[dependencies]
iron = "0.4"
router = "0.4"
logger = "0.2"
env_logger = "0.3"
rustc-serialize = "0.3"
chrono = { version = "0.2", features = ["rustc-serialize"] }
uuid = { version = "0.2", features = ["v4", "rustc-serialize"] }
```

`iron` provides us with a web server. `router` helps with routing. `logger` and `env_logger` allow us to log requests.
`rustc-serialize` allows us to serialize structs to JSON on Stable Rust. `chrono` provides us with an interface
for date and time representations that works easily with `rustc-serialize`. Finally, we use `uuid` to be able
to generate unique ids for posts.

## Modeling our data

We will be modeling posts which have the following properties:

* A summary or a short title
* A longer content
* An author handle
* The date and time of posting in the UTC timezone
* A unique identifier

Create a file called `model.rs` and add the following definitions.

```rust
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
```

The `Author` struct is not strictly necessary here, but I think it's cleaner to
reserve this for the future. Most of the above code should be fairly self explanatory.
Deriving `RustcEncodable` and `RustcDecodable` allows us to have the `rustc-serialize` library
automatically encode and decode objects to and from JSON.

## Saving our data

For this tutorial, a real database would be out of scope. That's why we are going to store everything
in memory instead. Our database will be a simple `Vec<Post>` that is available in all of our web handlers.
We will be able to add posts to this database, or get all the posts from the database.

Create a file called `database.rs` and add the following, self-explanatory code.

```rust
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
```

## Handling HTTP requests

Now that we've defined our data and a simple in-memory data store, we will be looking
at the meat of the application: handling the actual web requests.

Start by creating a file `handlers.rs`. We will need the following `use` statements:

```rust
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
```

### Macros

A lot of code has to deal with JSON encoding and decoding, mutexes or getting parameters from
an HTTP request. That is why we will be creating three macros:

* A first macro that allows us to match a `Result` to get the inner value, or return an `InternalServerError`,
or some other HTTP error. This macro is similar to the `try` macro from the Rust standard library.
* A second macro that allows us to write `lock!(mutex)` instead of `mutex.lock().unwrap()`. We will be
calling `unwrap` on all of the locked mutexes in this code for simplicity's sake. I also haven't figured out
yet why one wouldn't call `unwrap`, as I'm not sure it's possible to recover from such an error. If you know why,
please leave a comment.
* A third macro to easily get a parameter from an HTTP GET request, or, if the parameter is not present,
return `BadRequest`, or if something went wrong, return `InternalServerError`.

```rust
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
```

This first macro has two variants: the first returns an `InternalServerError` on failure,
the second one an error that has been provided.

```rust
macro_rules! lock {
    ( $e:expr ) => { $e.lock().unwrap() }
}
````

This very simple macro is simply a syntactic replacement for `.lock().unwrap()`.

```rust
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
```

This final macro allows us to obtain a parameter from an HTTP GET request.
We first make sure that we can get our `Router`. If this is not possible, something terrible
went wrong, so we return `InternalServerError`. If we cannot find the required parameter,
the users have sent a `BadRequest`.

So far, this is all fairly simple. These macros will make our lives a lot easier when writing the actual
request handlers.

### Request handlers

Now, let's have a look at the actual handlers. We will first create a struct
that will contain all handlers:

```rust
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
```

As you can see, each handler has access to our data store through an `Arc<Mutex<Database>>`.
Why is this? Well, we need the following features for our data store:

* We want it to be accessible in all handlers (this should make you think: `&` or `Rc`)
* We want changes from one handler to be visible in other handlers immediately (this should make you think `RefCell`)
* We need it to be thread safe. The thread safe equivalent of `Rc<RefCell>>` is `Arc<Mutex>`.

In case you don't know what `Arc` or `Mutex` are, you should have a look at the excellent sections
on them in the [Rust documentation](https://doc.rust-lang.org/std/sync/struct.Arc.html).

#### Feed handler

Let's start with the easiest handler: the one that returns a list of all posts.

```rust
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
```

The final `impl` block shows us how to implement `iron::Handler`: we handle a request
and return a response. The response itself is wrapped in an `IronResult`, but the documentation
on this is not very clear and as far as I know it's okay to always return `Ok`, as long as you
return the appropriate HTTP status code. Maybe if you're doing something more fancy, you might need
to return `Err`.

We immediately see two of our macros in action: `try_handler` and `lock`. We lock the database,
obtain the posts, encode this as a JSON list and send this with `HTTP 200 OK`.

Without the macros, the code would look like this:

```rust
fn handle(&self, _: &mut Request) -> IronResult<Response> {
    let payload = match json::encode(self.database.lock().unwrap().posts()) {
        Ok(pl) => pl,
        Err(e) => return Ok(Response::with((status::InternalServerError, e.description()))),
    };
    Ok(Response::with((status::Ok, payload)))
}
```

As you can see, the macros do make this code a lot more clear.

#### Make post handler

Now let's allow users to create posts.

```rust
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
```

The `handle` function is a bit more complicated now. First we obtain the payload from the request's body:
this is the data we are sending with the HTTP POST request. Then, we try to
decode the payload into a `Post` object, returning a `BadRequest` if the JSON is malformed.

Next, we add the post to the database and return 201 Created along with the original payload.

#### Post handler

We also want to be able to view individual posts. For this, we need to have a URL parameter.
The handler looks like this:

```rust
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
            let payload = try_handler!(json::encode(&post), status::InternalServerError);
            Ok(Response::with((status::Ok, payload)))
        } else {
            Ok(Response::with((status::NotFound)))
        }
    }
}
```

Our handler has a special function `find_post` that looks through the database
for a post with a matching id. If we find one, we clone it and return it. Otherwise, we return `None`.

Note how we can call `map` on the `Option` resulting from `find` because `Option` implements
the `Iterator` trait, which is really cool and useful!

The handler itself first attempts to find the `id` parameter using our macro. If it finds this,
it tries to parse it to the UUID. If it is malformed, we return 400 Bad Request.

Finally, if we find a post, we encode it and return it with 200 OK. If we don't find it,
we return 404 Not Found.

Great! We have now defined all of our handlers. Only one thing remains to be done in this file.

### Middleware

Iron uses middleware. Middleware can be either BeforeMiddleware, AroundMiddleware or AfterMiddleware.

BeforeMiddleware can modify requests before they are handled, AroundMiddleware before and after and
AfterMiddleware after they are handled but before the response is sent.

In this case, we need an AfterMiddleware to make sure that our responses have the correct content type:
`application/json`. This is really simple:

```rust
pub struct JsonAfterMiddleware;

impl AfterMiddleware for JsonAfterMiddleware {
    fn after(&self, _: &mut Request, mut res: Response) -> IronResult<Response> {
        res.headers.set(ContentType::json());
        Ok(res)
    }
}
```

We simply update the headers on the response, and we're done.

## Putting it all together

We have now defined all of the code that is needed for our data, data store and handlers.

Now it is time to put everything together and have a running web application.

We should start by defining all of the external crates, models, and adding our `use` statements.

```rust
extern crate iron;
extern crate router;
extern crate logger;
extern crate env_logger;
extern crate rustc_serialize;
extern crate chrono;
extern crate uuid;

mod model;
mod database;
mod handlers;

use model::*;
use database::Database;
use handlers::*;

use iron::prelude::Chain;
use iron::Iron;
use router::Router;
use logger::Logger;
use uuid::Uuid;
```

Now, in the `main` function, we will start by initialising our logger and creating a pair of
BeforeMiddleware and AfterMiddleware that is required for the logger.

```rust
fn main() {
    env_logger::init().unwrap();
    let (logger_before, logger_after) = Logger::new(None);

    // ...
```

Now we create two first entries in our database to have something to show.

```rust
// ...
let mut database = Database::new();
let author = Author::new("Me");
let post = Post::new("First post",
                     "This is the first post ever",
                     &author,
                     chrono::offset::utc::UTC::now(),
                     Uuid::new_v4());
database.add_post(post);
let post = Post::new("My web app is now online",
                     "Today marks the day that this app is online!",
                     &author,
                     chrono::offset::utc::UTC::now(),
                     Uuid::new_v4());
database.add_post(post);
// ...
```

Note the construction of the posts: `Uuid::new_v4()` will generate a random UUID for each post.
We also use `chrono::offset::utc::UTC::now()` to simulate that these posts were created just now.

Now we instantiate our handlers and middleware:

```rust
// ...
let handlers = Handlers::new(database);
let json_content_middleware = JsonAfterMiddleware;
// ...
```

And now we have to define our routes. We will map each route to its handler and a different path:

```rust
// ...
let mut router = Router::new();
router.get("/feed", handlers.feed, "feed");
router.post("/post", handlers.make_post, "make_post");
router.get("/post/:id", handlers.post, "post");
// ...
```

We also need to define a Chain for our middleware: we need to make sure the logger middlewares
are the first and last (see the `logger` crate documentation), and we also need to add our JSON middleware.

```rust
// ...
let mut chain = Chain::new(router);
chain.link_before(logger_before); // Should be first!
chain.link_after(json_content_middleware);
chain.link_after(logger_after); // Should be last!
// ...
```

Finally, we can start our server!

```rust
    // ...
    Iron::new(chain).http("localhost:3000").unwrap();
}
```

We can run our server and see the logs by executing `RUST_LOG=logger=info cargo run` on the command line.

To try it out, you can use `curl`:

* To look at the feed: `curl -v localhost:3000/feed`
* To look at a specific post, find its id in the feed and then use `curl -v localhost:3000/post/<id>`
* To create a new post, make sure you get the JSON syntax right and then use `curl -v --data '<json>' localhost:3000/post`

## Where to go from here

You now have a working web application. It does not do much, however. Things you can do are:

* Have a look at the libraries provided by the Iron team ([Iron Common](https://github.com/iron-graveyard/common))
* Implement authentication, registering and logging in
* Implement a database using for example PostgreSQL
* Implement updating or deleting posts
* ...

If you are getting any errors, you can check out the complete code [here](https://github.com/m-decoster/iron-example).
