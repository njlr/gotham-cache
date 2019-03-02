extern crate tokio;
extern crate hyper;

#[macro_use]
extern crate gotham_derive;
extern crate serde;

#[macro_use]
extern crate serde_derive;

use std::io::Write;

use hyper::{Body, StatusCode};
use tokio::prelude::{Future, Stream};

use gotham::handler::HandlerFuture;
use gotham::handler::assets::FileOptions;
use gotham::helpers::http::response::create_empty_response;
use gotham::router::builder::{build_simple_router, DefineSingleRoute, DrawRoutes};
use gotham::state::{FromState, State};

#[derive(Deserialize, StateData, StaticResponseExtender)]
struct PathExtractor {
    id: String,
}

fn ac_put_handler(mut state: State) -> Box<HandlerFuture> {
    let path = PathExtractor::borrow_from(&state);

    let worker = tokio::fs::File::create("cache/ac".clone().to_owned() + &path.id)
        .and_then(|mut file| {
            Body::take_from(&mut state)
                .map_err(|err| panic!("Body error: {}", err))
                .for_each(move |chunk| file.write(&chunk).map(|_| ()))
                .and_then(|_| {
                    let res = create_empty_response(&state, StatusCode::OK);
                    Ok((state, res))
                })
        })
        .map_err(|err| panic!("IO error: {:?}", err));

    Box::new(worker)
}

fn cas_put_handler(mut state: State) -> Box<HandlerFuture> {
    let path = PathExtractor::borrow_from(&state);

    let worker = tokio::fs::File::create("cache/cas".clone().to_owned() + &path.id)
        .and_then(|mut file| {
            Body::take_from(&mut state)
                .map_err(|err| panic!("Body error: {}", err))
                .for_each(move |chunk| file.write(&chunk).map(|_| ()))
                .and_then(|_| {
                    let res = create_empty_response(&state, StatusCode::OK);
                    Ok((state, res))
                })
        })
        .map_err(|err| panic!("IO error: {:?}", err));

    Box::new(worker)
}

pub fn main() {
    let addr = "127.0.0.1:8080";
    println!("Listening for requests at http://{}", addr);
    gotham::start(
        addr,
        build_simple_router(|route| {

            route
                .get("/ac/*")
                .to_dir(
                    FileOptions::new("cache/ac")
                    .build(),
                );

            route
                .put("/ac/:id:[a-f0-9]{64}")
                .with_path_extractor::<PathExtractor>()
                .to(ac_put_handler);

            route
                .get("/cas/*")
                .to_dir(
                    FileOptions::new("cache/cas")
                    .build(),
                );

            route
                .put("/cas/:id:[a-f0-9]{64}")
                .with_path_extractor::<PathExtractor>()
                .to(cas_put_handler);
        }),
    )
}
