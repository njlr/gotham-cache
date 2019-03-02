extern crate tokio;
extern crate hyper;

#[macro_use]
extern crate gotham_derive;
extern crate serde;

#[macro_use]
extern crate serde_derive;

use std::io::Write;
use std::sync::RwLock;
use std::sync::Arc;
use std::collections::HashSet;

use futures::future::ok;

use tokio::prelude::{Future, Stream};

use hyper::{Body, StatusCode};

use gotham::handler::{Handler, HandlerFuture};
use gotham::handler::assets::{DirHandler, FileOptions};
use gotham::helpers::http::response::create_empty_response;
use gotham::router::builder::{build_simple_router, DefineSingleRoute, DrawRoutes};
use gotham::state::{FromState, State};

#[derive(Deserialize, StateData, StaticResponseExtender)]
struct PathExtractor {
    id: String,
}

struct CacheHandler {
    lock: Arc<RwLock<HashSet<String>>>,
    base_path: String,
    dir_handler: DirHandler,
}

impl CacheHandler {
    fn new (base_path: &str) -> CacheHandler {
        let options = FileOptions::new(base_path)
                .build();

        CacheHandler {
            lock: Arc::new(RwLock::new(HashSet::new())),
            base_path: base_path.to_string(),
            dir_handler: DirHandler::new(options),
        }
    }

    fn handle_get (&self, state: State) -> Box<HandlerFuture> {
        let path = PathExtractor::borrow_from(&state);

        let f = self.base_path.clone() + "/" + &path.id;

        let is_in_flight = self.lock.read().unwrap().contains(&f);

        if is_in_flight {
            let res = create_empty_response(&state, StatusCode::NOT_FOUND);

            Box::new(ok((state, res)))
        } else {
            self.dir_handler.clone().handle(state)
        }
    }

    fn handle_put (&self, mut state: State) -> Box<HandlerFuture> {
        let path = PathExtractor::borrow_from(&state);

        let f = self.base_path.clone() + "/" + &path.id;

        let is_in_flight = self.lock.read().unwrap().contains(&f);

        if is_in_flight {
            let res = create_empty_response(&state, StatusCode::OK);

            Box::new(ok((state, res)))
        } else {
            self.lock.write().unwrap().insert(f.clone());

            let lock = self.lock.clone();
            let f_then = f.clone();
            let f_err = f.clone();
            let lock_err = self.lock.clone();

            let worker = tokio::fs::File::create(f)
                .and_then(move |mut file| {
                    Body::take_from(&mut state)
                        .map_err(|err| panic!("Body error: {}", err))
                        .for_each(move |chunk| file.write(&chunk).map(|_| ()))
                        .and_then(move |_| {
                            let res = create_empty_response(&state, StatusCode::OK);

                            lock.write().unwrap().remove(&f_then);

                            Ok((state, res))
                        })
                })
                .map_err(move |err| {
                    lock_err.write().unwrap().remove(&f_err);

                    panic!("IO error: {:?}", err)
                });

            Box::new(worker)
        }
    }
}

pub fn main() {

    let addr = "127.0.0.1:8080";

    println!("Listening for requests at http://{}", addr);

    let ac_handler = Arc::new(CacheHandler::new("cache/ac"));
    let cas_handler = Arc::new(CacheHandler::new("cache/cas"));

    gotham::start(
        addr,
        build_simple_router(|route| {
            {
                let ac_handler = ac_handler.clone();

                route
                    .get("/ac/*")
                    .to_new_handler(move || {
                        let ac_handler = ac_handler.clone();
                        Ok(move |state: State| {
                            ac_handler.handle_get(state)
                        })
                    });
            }

            {
                let ac_handler = ac_handler.clone();

                route
                    .put("/ac/:id:[a-f0-9]{64}")
                    .with_path_extractor::<PathExtractor>()
                    .to_new_handler(move || {
                        let ac_handler = ac_handler.clone();
                        Ok(move |state: State| {
                            ac_handler.handle_put(state)
                        })
                    });
            }

            {
                let cas_handler = cas_handler.clone();

                route
                    .get("/cas/*")
                    .to_new_handler(move || {
                        let cas_handler = cas_handler.clone();
                        Ok(move |state: State| {
                            cas_handler.handle_get(state)
                        })
                    });
            }

            {
                let cas_handler = cas_handler.clone();

                route
                    .put("/cas/:id:[a-f0-9]{64}")
                    .with_path_extractor::<PathExtractor>()
                    .to_new_handler(move || {
                        let cas_handler = cas_handler.clone();
                        Ok(move |state: State| {
                            cas_handler.handle_put(state)
                        })
                    });
            }
        }),
    )
}
