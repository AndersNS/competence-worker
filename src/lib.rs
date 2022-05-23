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

#[allow(unused)]
#[durable_object]
pub struct Competencies {
    competencies: Vec<CompetencyRating>,
    state: State,
    env: Env, // access `Env` across requests, use inside `fetch`
}

#[durable_object]
impl DurableObject for Competencies {
    fn new(state: State, env: Env) -> Self {
        Self {
            state,
            env,
            competencies: vec![],
        }
    }

    async fn fetch(&mut self, req: Request) -> Result<Response> {
        match req.method() {
            Method::Get => Response::ok(serde_json::to_string(&self.competencies)?),
            Method::Post => {
                self.competencies.push(CompetencyRating {
                    discipline_id: 1,
                    path_id: 1,
                    area_id: 1,
                    comp_id: 1,
                    rating: Rating::Interest(4),
                });

                Response::ok(serde_json::to_string(&self.competencies)?)
            }
            _ => Response::error("Method not allowed", 401),
        }
    }
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
        .on_async("/competencyrating/:id", |req, ctx| async move {
            if let Some(id) = ctx.param("id") {
                let namespace = ctx.durable_object("competencies")?;
                let stub = namespace.id_from_name(id)?.get_stub()?;
                return stub.fetch_with_request(req).await;
            }

            Response::error("Bad request", 400)
        })
        .get_async("/competency/:id", |_req, ctx| async move {
            if let Some(id) = ctx.param("id") {
                let namespace = ctx.kv("main")?;
                let value = namespace.get(id).text().await?;
                if let Some(val) = value {
                    let cors = get_cors();
                    return Response::ok(val)?.with_cors(&cors);
                }
                let cors = get_cors();
                return Response::error("Not found", 404)?.with_cors(&cors);
            }

            Response::error("Bad request", 400)
        })
        .post_async("/competency/:id", |mut req, ctx| async move {
            if let Some(id) = ctx.param("id") {
                let kv = ctx.kv("main")?;

                let ratings: Vec<CompetencyRating> = serde_json::from_str(&(req.text().await?))?;

                kv.put(id, ratings).unwrap().execute().await?;
                let cors = get_cors();
                return Response::ok("Ok")?.with_cors(&cors);
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
