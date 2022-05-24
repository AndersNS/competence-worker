use serde::{Deserialize, Serialize};
use worker::*;

mod utils;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

fn log_request(req: &Request) {
    console_log!(
        "{} - [{}], located at: {:?}, within: {}",
        Date::now().to_string(),
        req.path(),
        req.cf().coordinates().unwrap_or_default(),
        req.cf().region().unwrap_or("unknown region".into())
    );
}

#[derive(Copy, Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum Rating {
    Interest(i32),
    Competency(i32),
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub struct CompetencyRating {
    pub discipline_id: usize,
    pub path_id: usize,
    pub area_id: usize,
    pub comp_id: usize,
    pub rating: Rating,
}

fn get_cors() -> Cors {
    let cors = Cors::default().with_origins(vec!["*"]).with_methods(vec![
        Method::Get,
        Method::Head,
        Method::Post,
        Method::Options,
    ]);

    return cors;
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: worker::Context) -> Result<Response> {
    log_request(&req);

    // Optionally, get more helpful error messages written to the console in the case of a panic.
    utils::set_panic_hook();

    // Optionally, use the Router to handle matching endpoints, use ":name" placeholders, or "*name"
    // catch-alls to match on specific patterns. Alternatively, use `Router::with_data(D)` to
    // provide arbitrary data that will be accessible in each route via the `ctx.data()` method.
    let router = Router::new();

    // Add as many routes as your Worker needs! Each route will get a `Request` for handling HTTP
    // functionality and a `RouteContext` which you can use to  and get route parameters and
    // Environment bindings like KV Stores, Durable Objects, Secrets, and Variables.
    router
        .get("/", |_, _| Response::ok("Hello from Workers!"))
        .get_async("/competency/:id", |_req, ctx| async move {
            if let Some(id) = ctx.param("id") {
                let namespace = ctx.kv("main")?;
                let value = namespace.get(id).text().await?;
                if let Some(val) = value {
                    return Response::ok(val)?.with_cors(&get_cors());
                }
                return Response::error("Not found", 404)?.with_cors(&get_cors());
            }

            Response::error("Bad request", 400)
        })
        .post_async("/competency/:id", |mut req, ctx| async move {
            if let Some(id) = ctx.param("id") {
                let kv = ctx.kv("main")?;

                let ratings: Vec<CompetencyRating> = serde_json::from_str(&(req.text().await?))?;

                kv.put(id, ratings).unwrap().execute().await?;
                return Response::ok("Ok")?.with_cors(&get_cors());
            }

            Response::error("Bad request", 400)
        })
        .get("/worker-version", |_, ctx| {
            let version = ctx.var("WORKERS_RS_VERSION")?.to_string();
            Response::ok(version)
        })
        .run(req, env)
        .await
}
